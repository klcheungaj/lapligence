// A foreach loop inside a fork branch whose body holds a detached fork that
// names the loop variable: that fork shares the loop's one variable.
`timescale 1ns/1ns
module tb;
  int a[3];
  string keys[string];
  initial begin
    keys["p"] = "1";
    keys["r"] = "2";
    fork
      begin
        foreach (a[i]) fork
          #1 $display("(A) i=%0d", i);
        join_none
        foreach (keys[s]) fork
          #1 $display("(B) s=%s", s);
        join_none
        #2 $display("(C) branch done");
      end
    join_none
    #3 $finish;
  end
endmodule
