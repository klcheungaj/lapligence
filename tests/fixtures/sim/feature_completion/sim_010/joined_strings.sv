// SIM-010: branches of a fork-join share the enclosing automatic string with
// the suspended parent (SV 6.21, 9.3.2), so their writes are seen after join.
module tb;
  task automatic append();
    string s = "a";
    fork
      #1 s = {s, "b"};
      #2 s = {s, "c"};
    join
    $display("task %s", s);
  endtask

  initial begin
    automatic string s = "x";
    append();
    fork
      #1 s = {s, "y"};
      #2 s = {s, "z"};
    join
    $display("block %s", s);
    $finish;
  end
endmodule
