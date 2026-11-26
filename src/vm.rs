use std::collections::HashMap;

use crate::chunk::{Bytecode, Chunk, Op};
// The same error type and the same frame limit as the tree-walker, deliberately shared.
// Spec section 5 fixes both, and the two engines are required to produce identical output.
use crate::interpreter::{RuntimeError, MAX_FRAMES};
use crate::value::Value;

/// One active call: which chunk it is running, how far through it is, and where its slice
/// of the stack begins. This is the activation record of week 13's Module A, made of two
/// `usize`s and an index; there is nothing else in it, because Kobo has no closures.
struct Frame {
    /// An index into `Bytecode::functions`, or `None` for the script itself.
    function: Option<usize>,
    ip: usize,
    /// Stack index of local slot 0. The callee's own value sits just below it.
    base: usize,
}

/// Run compiled bytecode. The VM owns it: nothing needs the chunks afterwards, and owning
/// them keeps a lifetime out of every signature in this file.
pub fn run(bytecode: Bytecode) -> Result<(), RuntimeError> {
    let mut vm = Vm {
        bytecode,
        stack: Vec::new(),
        globals: HashMap::new(),
        frames: vec![Frame {
            function: None,
            ip: 0,
            base: 0,
        }],
    };
    vm.run()
}

struct Vm {
    bytecode: Bytecode,
    /// Values in flight and the locals of every active call, in one contiguous vector.
    /// The interpreter kept those two things apart: a `Vec` of `HashMap`s for the names
    /// and the Rust stack for the arithmetic. Merging them is what makes this fast.
    stack: Vec<Value>,
    globals: HashMap<String, Value>,
    frames: Vec<Frame>,
}

impl Vm {
    fn run(&mut self) -> Result<(), RuntimeError> {
        loop {
    let function = self.frame().function;
    let ip = self.frame().ip;

    let op = self.chunk(function).code[ip];
    let line = self.chunk(function).lines[ip];

    self.frame_mut().ip += 1;

    match op {
        Op::Constant(index) => {
            let value =
                self.chunk(function).constants[index].clone();

            self.stack.push(value);
        }

        Op::Nil => {
            self.stack.push(Value::Nil);
        }

        Op::True => {
            self.stack.push(Value::Bool(true));
        }

        Op::False => {
            self.stack.push(Value::Bool(false));
        }

        Op::Pop => {
            self.pop();
        }

        Op::DefineGlobal(index) => {
            let name = self.name(function, index);
            let value = self.pop();

            self.globals.insert(name, value);
        }

        Op::GetGlobal(index) => {
            let name = self.name(function, index);

            match self.globals.get(&name) {
                Some(value) => {
                    self.stack.push(value.clone());
                }

                None => {
                    return Err(undefined(line, &name));
                }
            }
        }

        Op::SetGlobal(index) => {
            let name = self.name(function, index);
            let value = self.peek().clone();

            if !self.globals.contains_key(&name) {
                return Err(undefined(line, &name));
            }

            self.globals.insert(name, value);
        }

        Op::GetLocal(slot) => {
            let index = self.frame().base + slot;
            let value = self.stack[index].clone();

            self.stack.push(value);
        }

        Op::SetLocal(slot) => {
            let index = self.frame().base + slot;
            let value = self.peek().clone();

            self.stack[index] = value;
        }

        Op::Equal => {
            self.binary_value(|a, b| Value::Bool(a == b));
        }

        Op::NotEqual => {
            self.binary_value(|a, b| Value::Bool(a != b));
        }

        Op::Greater => {
            self.compare(line, |a, b| a > b)?;
        }

        Op::GreaterEqual => {
            self.compare(line, |a, b| a >= b)?;
        }

        Op::Less => {
            self.compare(line, |a, b| a < b)?;
        }

        Op::LessEqual => {
            self.compare(line, |a, b| a <= b)?;
        }

        Op::Add => {
            let (a, b) = self.pop_two();

            match (a, b) {
                (Value::Number(x), Value::Number(y)) => {
                    self.stack.push(Value::Number(x + y));
                }

                (Value::Str(x), Value::Str(y)) => {
                    self.stack.push(Value::Str(x + &y));
                }

                _ => {
                    return Err(error(
                        line,
                        "Operands must be two numbers or two strings.",
                    ));
                }
            }
        }

        Op::Subtract => {
            self.arith(line, |a, b| a - b)?;
        }

        Op::Multiply => {
            self.arith(line, |a, b| a * b)?;
        }

        Op::Divide => {
            let (a, b) = self.numbers(line)?;

            if b == 0.0 {
                return Err(error(line, "Division by zero."));
            }

            self.stack.push(Value::Number(a / b));
        }

        Op::Not => {
            let value = self.pop();

            self.stack.push(Value::Bool(!value.is_truthy()));
        }

        Op::Negate => {
            match self.pop() {
                Value::Number(n) => {
                    self.stack.push(Value::Number(-n));
                }

                _ => {
                    return Err(error(
                        line,
                        "Operand must be a number.",
                    ));
                }
            }
        }

        Op::Print => {
            let value = self.pop();
            println!("{}", value);
        }

        Op::Jump(target) => {
            self.frame_mut().ip = target;
        }

        Op::JumpIfFalse(target) => {
            if !self.peek().is_truthy() {
                self.frame_mut().ip = target;
            }
        }

        Op::Call(argc) => {
            self.call(argc, line)?;
        }

        Op::Return => {
            let value = self.pop();

            if self.frames.len() == 1 {
                return Ok(());
            }

            let base = self.frame().base;

            self.frames.pop();
            self.stack.truncate(base - 1);
            self.stack.push(value);
        }
    }
}
 }

    /// The callee is on the stack underneath its arguments, put there by the code that
    /// evaluated it. Nothing is moved: the arguments are already lying where the new
    /// frame's slots 0 upwards need them to be.
    fn call(&mut self, argc: usize, line: usize) -> Result<(), RuntimeError> {
        // TODO(you): spec 4.3 again, on a stack instead of a tree: the arguments are already
        //            lying where the callee's slots need them.
        todo!("Vm::call")
    }

    // --- primitives -------------------------------------------------------------------

    fn chunk(&self, function: Option<usize>) -> &Chunk {
        match function {
            None => &self.bytecode.script,
            Some(i) => &self.bytecode.functions[i],
        }
    }

    fn name(&self, function: Option<usize>, index: usize) -> String {
        self.chunk(function).constants[index].to_string()
    }

    fn frame(&self) -> &Frame {
        self.frames.last().expect("a frame is always running")
    }

    fn frame_mut(&mut self) -> &mut Frame {
        self.frames.last_mut().expect("a frame is always running")
    }

    fn pop(&mut self) -> Value {
        self.stack.pop().expect("the compiler balances the stack")
    }

    fn peek(&self) -> &Value {
        self.stack.last().expect("the compiler balances the stack")
    }

    fn pop_two(&mut self) -> (Value, Value) {
        let b = self.pop();
        let a = self.pop();
        (a, b)
    }

    fn binary_value(&mut self, f: fn(Value, Value) -> Value) {
        let (a, b) = self.pop_two();
        self.stack.push(f(a, b));
    }

    fn numbers(&mut self, line: usize) -> Result<(f64, f64), RuntimeError> {
        match self.pop_two() {
            (Value::Number(a), Value::Number(b)) => Ok((a, b)),
            _ => Err(error(line, "Operands must be numbers.")),
        }
    }

    fn arith(&mut self, line: usize, f: fn(f64, f64) -> f64) -> Result<(), RuntimeError> {
        let (a, b) = self.numbers(line)?;
        self.stack.push(Value::Number(f(a, b)));
        Ok(())
    }

    fn compare(&mut self, line: usize, f: fn(f64, f64) -> bool) -> Result<(), RuntimeError> {
        let (a, b) = self.numbers(line)?;
        self.stack.push(Value::Bool(f(a, b)));
        Ok(())
    }
}

fn undefined(line: usize, name: &str) -> RuntimeError {
    error(line, &format!("Variable '{}' is not defined.", name))
}

fn error(line: usize, message: &str) -> RuntimeError {
    RuntimeError {
        line,
        message: message.to_string(),
    }
}
