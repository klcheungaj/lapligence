// SIM-003 composition: whole-record and member reads of a native record in
// continuous assignments and always_comb, plus an event control on a packed
// member. IEEE 1800-2009 9.2.2.2, 9.4.2, 10.3, 13.4; see readme.md.
module tb;
  typedef struct {string s; int n; real r;} T;
  T r;
  int total, comb, edges;

  function automatic int weight(input T v);
    return v.n * 10 + v.s.len();
  endfunction

  assign total = weight(r);
  always_comb comb = weight(r) + int'(r.r);
  always @(r.n) edges++;

  initial begin
    #1 r = '{"ab", 1, 0.0};
    #1 $display("%0d %0d %0d", total, comb, edges);
    r.n = 2;
    #1 $display("%0d %0d %0d", total, comb, edges);
    r.s = "abcd";
    #1 $display("%0d %0d %0d", total, comb, edges);
    r.s = "abcd";
    r.r = 3.0;
    #1 $display("%0d %0d %0d", total, comb, edges);
    r = '{"z", 0, 0.5};
    #1 $display("%0d %0d %0d", total, comb, edges);
    $finish(0);
  end
endmodule
