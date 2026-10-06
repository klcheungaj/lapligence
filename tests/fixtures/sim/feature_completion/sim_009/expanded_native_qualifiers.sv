// SIM-009: an expanded task's `iff` qualifier reads its string and chandle
// formals; each control resumes only on an edge whose qualifier is true
// (SV 9.4.2).
module tb;
  logic level = 0;
  chandle none;

  task automatic gate(input string tag);
    @(posedge level iff tag == "go");
    $display("%s %0d", tag, $time);
  endtask

  task automatic gate_null(input chandle h);
    @(negedge level iff h == null);
    $display("null %0d", $time);
  endtask

  initial fork gate("go"); gate("stop"); gate_null(none); join_none
  initial begin #1 level = 1; #1 level = 0; #1 level = 1; #1 $finish; end
endmodule
