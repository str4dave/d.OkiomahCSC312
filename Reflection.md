REFLECTION.md

1\. Number literals with decimals:



The scanner decides whether a dot (.) is part of a decimal number inside the number() function. After reading the initial digits, it checks line 114:



**if self.peek() == '.' \&\& self.peek\_next().is\_ascii\_digit()**



This means the scanner only accepts the dot as part of the number when there is a digit immediately after it.

For example, 5.25 is scanned as one Number token because the dot is followed by 2. However, 5. does not satisfy the condition because there is no digit after the dot. Therefore, 5 is scanned as the number and . is left to be processed separately as an unrecognized character. This matches Section 1.4 because a fractional part must contain digits after the decimal point.



2\. Line numbers and EOF tokens:

Line numbers are updated whenever the scanner encounters a newline. **In scan\_token()**, line 80 contains:



**'\\n' => self.line += 1,**



The string() function also increases the line number when a string contains a newline.



However, the current **run()** function deliberately changes the line number before adding EOF. It takes the line number of the last real token and assigns it to self.line before creating the EOF token.



Therefore, if the file ends with two blank lines, the EOF token currently gets the last real token's line number, not the final physical line number. This is important because Section 6.1 requires EOF to represent the end of the actual scanned input, including trailing blank lines. So this current implementation may need to be checked against the exact Section 6.1 wording.



3\. 3. Test bug fix \& Git history



The commit **d0f9daf7be8da6fd2afbab09f3425952f3aa3997** shows the change made to **src/scanner.rs**. Before this commit, the run() function was still **todo!("run")**. The before commit is 2b27f9b, shown by the diff line index **2b27f9b..b52b50b**. The after commit is **d0f9daf7be8da6fd2afbab09f3425952f3aa3997**.



The main fix was replacing the unfinished **run()** function with code that scans tokens until the source ends and then adds EOF. It also makes EOF use the line number of the last real token.



The concept I misunderstood was how EOF's line number should be handled. I initially needed to account for the fact that trailing blank lines should not change the line number attached to EOF. The fix uses the last real token's line number instead.









