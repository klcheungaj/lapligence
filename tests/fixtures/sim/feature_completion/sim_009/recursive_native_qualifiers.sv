// SIM-009: a recursive task whose event qualifier reads its string formal
// runs on the typed call path; each activation waits on its own edge
// (SV 9.4.2, 13.5).
module tb;
  logic c = 0;
  task automatic r(input string t, input int n);
    @(posedge c iff t != "");
    $display("%s %0d %0d", t, n, $time);
    if (n > 0) r(t, n - 1);
  endtask
  initial r("x", 2);
  initial begin repeat (6) #1 c = ~c; $finish; end
endmodule
