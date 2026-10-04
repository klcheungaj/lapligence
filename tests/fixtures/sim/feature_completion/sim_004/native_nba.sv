// SIM-004: nonblocking writes to persistent strings and module records.
// IEEE 1800-2009 4.9.4 and 10.4.2: the RHS is evaluated at issue and the
// update is performed in a later NBA region, in issue order (4.6).
module tb;
  typedef struct {string s; int n; real r;} rec_t;
  string s, t, order, swap_a, swap_b, late, stage1, stage2, cur;
  rec_t ra, rb, rc;
  int slen;
  logic clk;

  // A content change of `s` re-evaluates this reader (9.2.2.2).
  always_comb slen = s.len();

  function automatic string make(string base, int k);
    string text;
    text = {base, "-", $sformatf("%0d", k)};
    return text;
  endfunction

  function automatic rec_t mk(string tag, int n);
    rec_t r;
    r.s = tag;
    r.n = n;
    r.r = real'(n) / 2.0;
    return r;
  endfunction

  // The queued value is an automatic formal's copy; it must survive the
  // activation's return before the NBA region.
  task automatic issue(input string v);
    s <= {v, "!"};
  endtask

  always @(posedge clk) begin
    stage1 <= cur;
    stage2 <= stage1;
  end

  initial begin
    clk = 0;
    s = "old";
    t = "src";
    s <= t;
    t = "changed";
    $display("1 %s %s", s, t);
    #1 $display("2 %s %0d", s, slen);

    issue("abc");
    #1 $display("3 %s", s);
    s <= make("m", 7);
    #1 $display("4 %s", s);

    order = "x";
    order <= "a";
    order <= "b";
    order <= {order, "c"};
    #1 $display("5 %s", order);

    swap_a = "A";
    swap_b = "B";
    swap_a <= swap_b;
    swap_b <= swap_a;
    #1 $display("6 %s %s", swap_a, swap_b);

    late = "p";
    late <= #2 "late";
    late <= "now";
    #1 $display("7 %s", late);
    #2 $display("8 %s", late);

    ra = '{"x", 3, 1.5};
    rb <= ra;
    ra.s = "y";
    $display("9 [%s] %0d", rb.s, rb.n);
    #1 $display("10 %s %0d %0.2f", rb.s, rb.n, rb.r);
    rc <= mk("fn", 4);
    #1 $display("11 %s %0d %0.2f", rc.s, rc.n, rc.r);
    rc.s <= "mem";
    rc.n <= 9;
    #1 $display("12 %s %0d", rc.s, rc.n);
    rb <= '{"pat", 5, 0.25};
    rb <= #1 rc;
    #1 $display("13 %s %0d %0.2f", rb.s, rb.n, rb.r);
    #1 $display("14 %s %0d", rb.s, rb.n);

    cur = "c0";
    #1 clk = 1;
    #1 clk = 0; cur = "c1";
    #1 clk = 1;
    #1 $display("15 %s %s", stage1, stage2);
    clk = 0; cur = "c2";
    #1 clk = 1;
    #1 $display("16 %s %s", stage1, stage2);
    $finish(0);
  end
endmodule
