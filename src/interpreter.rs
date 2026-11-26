use crate::ast::{Expr, Program, Resolved, Stmt};
use crate::environment::Environment;
use crate::token::{Token, TokenType};
use crate::value::Value;

pub struct RuntimeError {
    pub line: usize,
    pub message: String,
}

/// Why a statement stopped early. An error unwinds to `main`; a `return` unwinds only as
/// far as the call that started the function, which is why both travel the same way. Rust
/// gives `?` for free on a `Result`, and a `return` is exactly a non-local exit.
enum Flow {
    Error(RuntimeError),
    Return(Value),
}

type Exec<T> = Result<T, Flow>;

/// Kobo's call stack is the Rust call stack: `call` calls `execute` calls `evaluate`
/// calls `call`. Running off the end of it aborts the process with no diagnostic, so the
/// depth is capped instead. Measured on this toolchain, the real limit is near 1,500
/// frames; 500 leaves room for a deeply nested expression inside every one of them.
pub const MAX_FRAMES: usize = 500;

pub fn interpret(program: &Program) -> Result<(), RuntimeError> {
    let mut env = Environment::new();
    for stmt in &program.stmts {
        match execute(stmt, &mut env, program) {
            Ok(()) => {}
            // `return` outside any function ends the program (spec section 3.1).
            Err(Flow::Return(_)) => return Ok(()),
            Err(Flow::Error(e)) => return Err(e),
        }
    }
    Ok(())
}

fn execute(stmt: &Stmt, env: &mut Environment, program: &Program) -> Exec<()> {
    match stmt {
    Stmt::Expression(expr) => {
        evaluate(expr, env, program)?;
    }

    Stmt::Print(expr) => {
        let value = evaluate(expr, env, program)?;
        println!("{}", value);
    }

    Stmt::Var { name, initialiser, .. } => {
        let value = match initialiser {
            Some(expr) => evaluate(expr, env, program)?,
            None => Value::Nil,
        };

        env.declare(&name.lexeme, value);
    }

    Stmt::Block { body, .. } => {
        env.push_scope();

        for statement in body {
            if let Err(e) = execute(statement, env, program) {
                env.pop_scope();
                return Err(e);
            }
        }

        env.pop_scope();
    }

    Stmt::If {
        condition,
        then_branch,
        else_branch,
    } => {
        if evaluate(condition, env, program)?.is_truthy() {
            execute(then_branch, env, program)?;
        } else if let Some(branch) = else_branch {
            execute(branch, env, program)?;
        }
    }

    Stmt::While { condition, body } => {
        while evaluate(condition, env, program)?.is_truthy() {
            execute(body, env, program)?;
        }
    }

    Stmt::Function { index, name, .. } => {
        let value = Value::Function {
            index: *index,
            name: name.clone(),
        };

        env.declare(name, value);
    }

    Stmt::Return(value) => {
        let result = match value {
            Some(expr) => evaluate(expr, env, program)?,
            None => Value::Nil,
        };

        return Err(Flow::Return(result));
    }
}

Ok(())
    
}

fn evaluate(expr: &Expr, env: &mut Environment, program: &Program) -> Exec<Value> {
    match expr {
    Expr::Literal { value, .. } => {
        Ok(value.clone())
    }

    Expr::Grouping(inner) => {
        evaluate(inner, env, program)
    }

    Expr::Variable { name, at } => {
        let value = match at {
            Resolved::Global => {
                env.get_global(&name.lexeme)
            }

            Resolved::Local { depth, .. } => {
                env.get_at(*depth, &name.lexeme)
            }
        };

        match value {
            Some(v) => Ok(v.clone()),
            None => Err(undefined(name)),
        }
    }

    Expr::Assign { name, value, at } => {
        let result = evaluate(value, env, program)?;

        let assigned = match at {
            Resolved::Global => {
                env.assign_global(&name.lexeme, result.clone())
            }

            Resolved::Local { depth, .. } => {
                env.assign_at(*depth, &name.lexeme, result.clone())
            }
        };

        if assigned {
            Ok(result)
        } else {
            Err(undefined(name))
        }
    }

    Expr::Unary { op, right } => {
        let value = evaluate(right, env, program)?;

        match op.kind {
            TokenType::Bang => {
                Ok(Value::Bool(!value.is_truthy()))
            }

            TokenType::Minus => {
                match value {
                    Value::Number(n) => Ok(Value::Number(-n)),
                    _ => Err(err(op.line, "Operand must be a number.")),
                }
            }

            _ => unreachable!(),
        }
    }

    Expr::Binary { left, op, right } => {
        let l = evaluate(left, env, program)?;
        let r = evaluate(right, env, program)?;

        match op.kind {
            TokenType::Plus => {
                match (l, r) {
                    (Value::Number(a), Value::Number(b)) => {
                        Ok(Value::Number(a + b))
                    }

                    (Value::Str(a), Value::Str(b)) => {
                        Ok(Value::Str(a + &b))
                    }

                    _ => Err(err(
                        op.line,
                        "Operands must be two numbers or two strings.",
                    )),
                }
            }

            TokenType::Minus => {
                arith(l, r, op.line, |a, b| a - b)
            }

            TokenType::Star => {
                arith(l, r, op.line, |a, b| a * b)
            }

            TokenType::Slash => {
                let (a, b) = numbers(l, r, op.line)?;

                if b == 0.0 {
                    return Err(err(op.line, "Division by zero."));
                }

                Ok(Value::Number(a / b))
            }

            TokenType::EqualEqual => Ok(Value::Bool(l == r)),
            TokenType::BangEqual => Ok(Value::Bool(l != r)),

            TokenType::Greater => {
                compare(l, r, op.line, |a, b| a > b)
            }

            TokenType::GreaterEqual => {
                compare(l, r, op.line, |a, b| a >= b)
            }

            TokenType::Less => {
                compare(l, r, op.line, |a, b| a < b)
            }

            TokenType::LessEqual => {
                compare(l, r, op.line, |a, b| a <= b)
            }

            _ => unreachable!(),
        }
    }

    Expr::Logical { left, op, right } => {
        let l = evaluate(left, env, program)?;

        if op.kind == TokenType::Or {
            if l.is_truthy() {
                return Ok(l);
            }
        } else {
            if !l.is_truthy() {
                return Ok(l);
            }
        }

        evaluate(right, env, program)
    }

    Expr::Call { callee, paren, args } => {
        let function = evaluate(callee, env, program)?;
        let mut values = Vec::new();

        for arg in args {
            values.push(evaluate(arg, env, program)?);
        }

        match function {
            Value::Function { index, .. } => {
                call(index, values, paren.line, env, program)
            }

            _ => Err(err(paren.line, "This value is not a function.")),
        }
    }
}
    
    
}

/// Run a function body in a fresh frame. Everything about Kobo's scope rules is in these
/// twenty lines: the parameters are declared in the frame's own scope, the body can reach
/// past it only to the globals, and the frame is popped on every path out: normal
/// completion, `return`, or a runtime error.
fn call(
    index: usize,
    args: Vec<Value>,
    line: usize,
    env: &mut Environment,
    program: &Program,
) -> Exec<Value> {
    // TODO(you): spec 4.3: check arity, push a frame, declare the parameters, run the body, and
    //            pop the frame on all three ways out.
    todo!("call")
}

fn numbers(l: Value, r: Value, line: usize) -> Exec<(f64, f64)> {
    match (l, r) {
        (Value::Number(a), Value::Number(b)) => Ok((a, b)),
        _ => Err(err(line, "Operands must be numbers.")),
    }
}

fn arith(l: Value, r: Value, line: usize, f: fn(f64, f64) -> f64) -> Exec<Value> {
    let (a, b) = numbers(l, r, line)?;
    Ok(Value::Number(f(a, b)))
}

fn compare(l: Value, r: Value, line: usize, f: fn(f64, f64) -> bool) -> Exec<Value> {
    let (a, b) = numbers(l, r, line)?;
    Ok(Value::Bool(f(a, b)))
}

fn undefined(name: &Token) -> Flow {
    err(name.line, &format!("Variable '{}' is not defined.", name.lexeme))
}

fn err(line: usize, message: &str) -> Flow {
    Flow::Error(RuntimeError {
        line,
        message: message.to_string(),
    })
}
