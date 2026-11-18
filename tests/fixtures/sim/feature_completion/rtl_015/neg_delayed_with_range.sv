// IEEE 1800-2009 11.4.14.4: a `with` range is evaluated and checked at the
// write. An intra-assignment-delayed target is lowered as a static lvalue, so
// a runtime range there is a located limit (docs/known_issues.md).
module tb;
  logic [7:0] q [0:3];
  int j = 1;
  initial begin
    {>>{q with [j +: 2]}} = #1 16'h5566;
    $display("%h %h", q[1], q[2]);
    $finish;
  end
endmodule
