CA3 Reflection

Question 1 – Short-circuit Evaluation

In my interpreter, I implemented short-circuit evaluation in
src/interpreter.rs:243 to src/interpreter.rs:256. The first thing
the code does is evaluate the left side of the expression. For
the "or" operator, if the left value is truthy, it returns that
value immediately. For the "and" operator, if the left value is
falsey, it returns that value immediately. If neither of these
conditions is met, the right side is evaluated and returned.

For example, nil or "fallback" returns "fallback" because nil
is falsey, so the interpreter moves to the right side. Also,
0 and 1 returns 1 because zero is considered truthy in Kobo.
At first, returning true or false might seem like the normal
way to handle logical operators. However, this would cause
some tests to fail because Kobo expects the actual value of
one of the operands rather than just a boolean value.


Question 2 – Name Resolution and Stack Slots

In src/resolver.rs:88 to src/resolver.rs:100, the lookup()
function searches through the local variables starting from
the most recent declaration. Once it finds an initialised
variable with the matching name, it returns its depth and
stack slot. If it does not find a matching local variable,
the variable is treated as global.

The advantage of resolving a local variable to a stack slot
before execution is that the VM already knows where the
variable is stored. Instead of searching for the variable
using its name like the interpreter does, the VM can access
the value directly through its stack position.

In src/compiler.rs:333 to src/compiler.rs:339, the end_scope()
function produces a Pop instruction for every local variable
leaving a scope. If one less Pop instruction is produced,
an unwanted value remains on the stack, which can affect
the positions of other variables. Phase 4 tests can detect
this problem, while Phase 3 cannot because the interpreter
uses an environment instead of the VM's stack.


Question 3 – String Escape Sequences

My extension was to implement string escape sequences in
Kobo, which include \n, \t, \" and \\.

In src/scanner.rs:125 to src/scanner.rs:134, I modified the
scanner to recognise escape sequences without closing the
string when it encounters an escaped quotation mark. The
original token lexeme is still preserved.

In src/parser.rs:429 to src/parser.rs:457, the parser
converts the escape sequences into their actual characters
before creating a Value::Str.

I did not need to make any changes to the bytecode
instructions because both the interpreter and the VM use
the string value that has already been processed by the
parser.

One problem I faced while implementing this extension was
accidentally removing the identifier section in the primary()
function. This caused only 15 out of 29 Phase 3 tests to
pass. After identifying the problem and restoring the missing
section, I ran the tests again and got 29 out of 29. Both
extension tests also passed on the interpreter and the VM.

Incorrect Commit

Commit hash: eb196be

After implementing the escape sequences, the next condition
after the string section was:

if self.matches(&[TokenType::LParen])

The identifier section was missing, which prevented the
parser from properly handling variable and function names.

Corrected Commit

Commit hash: 641cbbef0ece1d6f7a08067af1f8767b7b675400

I restored the following section in the primary() function:

if self.matches(&[TokenType::Identifier]) {
    return Ok(Expr::Variable {
        name: self.previous(),
        at: Resolved::Global,
    });
}

This correction restored identifier handling while keeping
the string escape sequence extension working. After this
correction, all the public Phase 3 tests passed.