// SIM-029: a failed randomize() changes nothing (IEEE 1800-2009 18.6.3) and
// pre_randomize()/post_randomize() order (18.6.2): pre runs before the
// current values, modes and state are read, post only after a commit.
class item_c;
  rand bit [7:0] a;
  rand bit signed [7:0] b;
  static rand bit [3:0] s;
  bit [7:0] lim;
  bit freeze_b;
  int pre_calls, post_calls;
  bit [7:0] seen_in_post;
  constraint lim_c { a < lim; b < 0; s != 0; }
  function void pre_randomize();
    pre_calls++;
    lim = lim + 1;                 // state read after pre_randomize
    if (freeze_b) b.rand_mode(0);  // modes read after pre_randomize
  endfunction
  function void post_randomize();
    post_calls++;
    seen_in_post = a;
  endfunction
endclass

module tb;
  bit [7:0] p, q;
  initial begin
    static item_c it = new;
    int ok;

    it.a = 11; it.b = -5; item_c::s = 3;
    it.lim = 255;                   // pre makes it 0: a < 0 is unsatisfiable
    ok = it.randomize();
    $display("unsat: ok=%0d a=%0d b=%0d s=%0d pre=%0d post=%0d lim=%0d",
             ok, it.a, it.b, item_c::s, it.pre_calls, it.post_calls, it.lim);

    it.lim = 4;                     // pre makes it 5
    ok = it.randomize();
    $display("sat: ok=%0d a_ok=%0d b_ok=%0d s_ok=%0d pre=%0d post=%0d post_saw_a=%0d",
             ok, it.a < 5, it.b < 0, item_c::s != 0, it.pre_calls, it.post_calls,
             it.seen_in_post == it.a);

    it.b = 7;                       // b frozen by pre: b < 0 now fails
    it.freeze_b = 1;
    ok = it.randomize();
    $display("frozen by pre: ok=%0d b=%0d mode=%0d post=%0d", ok, it.b, it.b.rand_mode(), it.post_calls);

    p = 8'd40; q = 8'd41;
    ok = std::randomize(p, q) with { p > q; q > 254; };
    $display("std unsat: ok=%0d p=%0d q=%0d", ok, p, q);
    ok = std::randomize(p, q) with { p > q; q > 250; p != 255 || q != 254; };
    $display("std sat: ok=%0d holds=%0d", ok, p > q && q > 250 && !(p == 255 && q == 254));
    $finish;
  end
endmodule
