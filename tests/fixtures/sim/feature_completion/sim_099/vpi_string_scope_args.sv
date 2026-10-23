// Gate item G4: string literal actuals of `string` formals and scope actuals
// reach a VPI application as vpiConstant string arguments and module
// handles (IEEE 1800-2009 D.4, D.6, D.7, 36.3.2). `$show` is declared with
// --define-system-task 'task $show(string s, logic [7:0] b)'.
module tb;
  sub u_sub();
  initial begin
    $list(tb);
    $list(u_sub);
    $input("cmds.txt");
    $log("run.log");
    $show("text", 8'h41);
    $finish;
  end
endmodule

module sub;
endmodule
