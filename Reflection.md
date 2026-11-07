Reflection 2
Q1:which lines of your if_statement resolve the dangling else, why the recursion decides that rather than the grammar, and what your parser prints for if (false) if (true) print "a"; else print "b";.

Code Citation: src/parser.rs:166-177 (in the if_statement function)

When writing an if  statement inside another if statement, it can be confusing to know which if else belongs to. In our parser, this is handled naturally through the act of recursion. When the parser reads the inner if statement, it checks for an else immediately after it. This means the else is connected to the closet unmatched if statement. For example, in if (false) if (true) print "a"; else print "b";, the else belongs to the inner if, not the outer one. This makes the parser handle nested if statements correctly

Q2: the line in assignment where you check what the left side turned out to be, why that can’t happen earlier, and the message that 1 = 2; produces.

Code Citation: src/parser.rs:207 (in the assignment function)

Assignment is difficult to identify immediately because the parser cannot know if a variable is being used normally or if it is part of an assignment. For example, a could be a normal variable or the start of a = 2;. To solve this, our parser first reads the entire expression and then checks if an equals sign follows. If the expression cannot be assigned to, such as 1 = 2;, it reports the error: Cannot assign to this expression. This helps the parser handle assignments correctly.


Q3: Your synchronise, which tokens you stop on and why, and one test in tests/phase-2/invalid/ whose output changes if you delete the call to it.
 
Code Citation: src/parser.rs:371-389 (in the synchronise function)

The synchronise() function helps the parser recover when it finds a syntax error. Instead of stopping completely or showing many errors, it skips the incorrect tokens until it finds a safe point to continue. These points include semicolons or keywords such as fun, var, if, while, print, and return. Without this function, one small mistake could cause many more error messages because the parser would struggle to understand the remaining code. This function helps reduce unnecessary errors and allows the parser to continue reading the rest of the program.

