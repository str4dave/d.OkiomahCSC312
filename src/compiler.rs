use crate::ast::{Expr, Program, Resolved, Stmt};
use crate::chunk::{Bytecode, Chunk, Op};
use crate::token::TokenType;
use crate::value::Value;

/// AST → bytecode. One pass, no intermediate graph: the tree is already the structure a
/// code generator wants, so walking it and emitting as we go is enough for a language
/// this size.
///
/// **Name resolution is not done here any more.** `resolver.rs` runs first and writes
/// every local's stack slot into the tree, so this file emits `GET_LOCAL 3` by reading a
/// number rather than by keeping its own scope stack and searching it. That used to be
/// two implementations of one rule, the resolver's and this one's, which had to agree
/// and had nothing checking that they did.
pub fn compile(program: &Program) -> Bytecode {
    let functions = program
        .functions
        .iter()
        .map(|f| {
            // Parameters are the frame's first locals, in order, so argument `i` is
            // already in slot `i` when the body starts; the resolver gave them those
            // slots and this compiler never has to know it.
            let mut c = Compiler::new(Chunk::new(f.name.clone(), f.params.len()), program);
            c.in_function = true;
            c.statements(&f.body);
            c.finish(f.line)
        })
        .collect();

    let mut script = Compiler::new(Chunk::new("script".to_string(), 0), program);
    script.statements(&program.stmts);
    let line = script.chunk.last_line();
    Bytecode {
        script: script.finish(line),
        functions,
    }
}

struct Compiler<'a> {
    chunk: Chunk,
    program: &'a Program,
    /// A function body's declarations are locals even at its outermost level; the
    /// script's are globals. That is the only thing this file still needs to know about
    /// scope, and the resolver decides everything else.
    in_function: bool,
}

impl<'a> Compiler<'a> {
    fn new(chunk: Chunk, program: &'a Program) -> Self {
        Compiler {
            chunk,
            program,
            in_function: false,
        }
    }

    /// Every chunk ends by returning `nil`, so falling off the end of a function body
    /// returns `nil` (spec section 4.3) without the VM needing a special case.
    fn finish(mut self, line: usize) -> Chunk {
        self.chunk.write(Op::Nil, line);
        self.chunk.write(Op::Return, line);
        self.chunk
    }

    fn statements(&mut self, stmts: &[Stmt]) {
        for stmt in stmts {
            self.statement(stmt);
        }
    }

    // --- statements -------------------------------------------------------------------

    fn statement(&mut self, stmt: &Stmt) {
        match stmt {
    Stmt::Expression(expr) => {
        let line = self.expression(expr);
        self.chunk.write(Op::Pop, line);
    }

    Stmt::Print(expr) => {
        let line = self.expression(expr);
        self.chunk.write(Op::Print, line);
    }

    Stmt::Var {
        name,
        initialiser,
        slot,
    } => {
        match initialiser {
            Some(expr) => {
                self.expression(expr);
            }

            None => {
                self.chunk.write(Op::Nil, name.line);
            }
        }

        self.declare(&name.lexeme, *slot, name.line);
    }

    Stmt::Block { body, locals } => {
        self.statements(body);
        self.end_scope(*locals);
    }

    Stmt::If {
        condition,
        then_branch,
        else_branch,
    } => {
        let line = self.expression(condition);

        let false_jump =
            self.chunk.write(Op::JumpIfFalse(0), line);

        self.chunk.write(Op::Pop, line);
        self.statement(then_branch);

        let end_jump =
            self.chunk.write(Op::Jump(0), line);

        self.patch(false_jump);
        self.chunk.write(Op::Pop, line);

        if let Some(branch) = else_branch {
            self.statement(branch);
        }

        self.patch(end_jump);
    }

    Stmt::While { condition, body } => {
        let start = self.chunk.code.len();
        let line = self.expression(condition);

        let exit_jump =
            self.chunk.write(Op::JumpIfFalse(0), line);

        self.chunk.write(Op::Pop, line);
        self.statement(body);
        self.chunk.write(Op::Jump(start), line);

        self.patch(exit_jump);
        self.chunk.write(Op::Pop, line);
    }

    Stmt::Function { index, name, slot } => {
        let line = self.program.functions[*index].line;

        let value = Value::Function {
            index: *index,
            name: name.clone(),
        };

        let constant = self.chunk.constant(value);

        self.chunk.write(Op::Constant(constant), line);
        self.declare(name, *slot, line);
    }

    Stmt::Return(value) => {
        let line = match value {
            Some(expr) => self.expression(expr),

            None => {
                let line = self.chunk.last_line();
                self.chunk.write(Op::Nil, line);
                line
            }
        };

        self.chunk.write(Op::Return, line);
    }
}
        
    }

    // --- expressions ------------------------------------------------------------------

    /// Emits the code for `expr` and returns the line the caller should attribute its own
    /// follow-up instruction to.
    fn expression(&mut self, expr: &Expr) -> usize {
        match expr {
    Expr::Literal { value, line } => {
        let op = match value {
            Value::Nil => Op::Nil,
            Value::Bool(true) => Op::True,
            Value::Bool(false) => Op::False,

            _ => {
                let index = self.chunk.constant(value.clone());
                Op::Constant(index)
            }
        };

        self.chunk.write(op, *line);
        *line
    }

    Expr::Grouping(inner) => {
        self.expression(inner)
    }

    Expr::Variable { name, at } => {
        let op = match at {
            Resolved::Global => {
                let index = self.name_constant(&name.lexeme);
                Op::GetGlobal(index)
            }

            Resolved::Local { slot, .. } => {
                Op::GetLocal(*slot)
            }
        };

        self.chunk.write(op, name.line);
        name.line
    }

    Expr::Assign { name, value, at } => {
        self.expression(value);

        let op = match at {
            Resolved::Global => {
                let index = self.name_constant(&name.lexeme);
                Op::SetGlobal(index)
            }

            Resolved::Local { slot, .. } => {
                Op::SetLocal(*slot)
            }
        };

        self.chunk.write(op, name.line);
        name.line
    }

    Expr::Unary { op, right } => {
        self.expression(right);

        let instruction = match op.kind {
            TokenType::Bang => Op::Not,
            TokenType::Minus => Op::Negate,
            _ => unreachable!(),
        };

        self.chunk.write(instruction, op.line);
        op.line
    }

    Expr::Binary { left, op, right } => {
        self.expression(left);
        self.expression(right);

        let instruction = match op.kind {
            TokenType::Plus => Op::Add,
            TokenType::Minus => Op::Subtract,
            TokenType::Star => Op::Multiply,
            TokenType::Slash => Op::Divide,
            TokenType::EqualEqual => Op::Equal,
            TokenType::BangEqual => Op::NotEqual,
            TokenType::Greater => Op::Greater,
            TokenType::GreaterEqual => Op::GreaterEqual,
            TokenType::Less => Op::Less,
            TokenType::LessEqual => Op::LessEqual,
            _ => unreachable!(),
        };

        self.chunk.write(instruction, op.line);
        op.line
    }

    Expr::Logical { left, op, right } => {
        self.expression(left);

        if op.kind == TokenType::And {
            let jump =
                self.chunk.write(Op::JumpIfFalse(0), op.line);

            self.chunk.write(Op::Pop, op.line);
            self.expression(right);
            self.patch(jump);
        } else {
            let false_jump =
                self.chunk.write(Op::JumpIfFalse(0), op.line);

            let end_jump =
                self.chunk.write(Op::Jump(0), op.line);

            self.patch(false_jump);

            self.chunk.write(Op::Pop, op.line);
            self.expression(right);
            self.patch(end_jump);
        }

        op.line
    }

    Expr::Call { callee, paren, args } => {
        self.expression(callee);

        for arg in args {
            self.expression(arg);
        }

        self.chunk.write(Op::Call(args.len()), paren.line);
        paren.line
    }
}

    // --- names ------------------------------------------------------------------------

    /// At the top level a name is global and stays a name. Inside any scope it is a local,
    /// and the value already sitting on the stack *is* the variable; declaring it is
    /// nothing more than agreeing which slot it occupies.
    /// A global is written into the globals map; a local is already on the stack in the
    /// slot the resolver gave it, so declaring one emits nothing. `slot` is the
    /// resolver's answer and `in_function` is the one case it cannot express on its own:
    /// the script's outermost declarations are globals and a function body's are not.
    fn declare(&mut self, name: &str, slot: Option<usize>, line: usize) {
        if slot.is_none() && !self.in_function {
    let index = self.name_constant(name);
    self.chunk.write(Op::DefineGlobal(index), line);
    }

    fn name_constant(&mut self, name: &str) -> usize {
        self.chunk.constant(Value::Str(name.to_string()))
    }
}

    /// Leaving a scope pops its locals off the stack, one `Pop` each. How many is the
    /// resolver's count, carried on the `Block`; this file no longer tracks scopes to
    /// work it out a second time. The interpreter dropped a whole `HashMap` here; the VM
    /// adjusts a stack pointer.
    fn end_scope(&mut self, locals: usize) {
        let line = self.chunk.last_line();

        for _ in 0..locals {
        self.chunk.write(Op::Pop, line);    
    }

    fn patch(&mut self, at: usize) {
        let target = self.chunk.code.len();

        match self.chunk.code[at] {
    Op::Jump(_) => {
        self.chunk.code[at] = Op::Jump(target);
    }

    Op::JumpIfFalse(_) => {
        self.chunk.code[at] = Op::JumpIfFalse(target);
    }

    _ => unreachable!(),
}
}
