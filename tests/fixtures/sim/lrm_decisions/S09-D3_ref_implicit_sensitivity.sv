// Decision S09-D3: implicit `@*` sensitivity of a statement that reads a
// `ref` formal includes the variable the actual names.
//
// IEEE 1800-2009 9.4.2.2 (SystemVerilog-1800-2009.txt L11892-11894): "The
//   implicit event_expression, @*, is a convenient shorthand that eliminates
//   these problems by adding all nets and variables that are read by the
//   statement (which can be a statement group) of a procedural_timing_
//   control_statement to the event_expression."
// IEEE 1800-2009 13.5.2 (L19138-19140): "both the caller and the subroutine
//   share the same representation of the argument".
//
// llg reading: the text speaks of nets and variables; llg treats a `ref`
// formal as the variable its actual names.
module tb;
  int level;

  task automatic follow(ref int r, input int n);
    @* if (r > 0) $display("follow %0d sees %0d at %0d", n, r, $time);
    if (n > 0) follow(r, n - 1);
  endtask

  initial begin
    fork
      follow(level, 1);
    join_none
    #1 level = 1;
    #1 level = 2;
    #1 $finish;
  end
endmodule
