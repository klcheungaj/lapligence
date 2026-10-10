// SIM-028 A03: writable legacy seeds (IEEE 1800-2009 20.15.1-20.15.2,
// IEEE 1364-2001 17.9.1-17.9.2). The seed is an inout 32-bit integer of the
// Annex N code: it is read from and written back to the named variable,
// whatever its storage, width or the formal it reaches the call through.
// Expected values come from annex_n_reference.c vectors (seed 1 -> result
// -2147414528, seed 69070; seed -1 -> 2147415551, seed -69068; seed 0 ->
// 303379748, seed -1844104698; $dist_uniform from seed 7 -> 0 then, with
// bounds -5..5, 3 and seed 483484 then -965981971) and the width rules of S28-D2/S28-D3.
class holder_c;
  integer seed;
endclass

module tb;
  integer seed, r, other;
  integer seeds[0:2];
  time wide;
  reg [47:0] r48;
  reg signed [15:0] s16;
  reg [15:0] u16;
  holder_c h;

  task automatic by_ref(ref integer s, output integer v);
    v = $random(s);
  endtask

  task automatic by_inout(inout integer s, output integer v);
    v = $dist_uniform(s, 0, 10);
  endtask

  // `s` and `v` may name the same variable: the seed is written back first,
  // then the result is assigned.
  task automatic aliased(ref integer s, ref integer v);
    v = $random(s);
  endtask

  initial begin
    seed = 1;
    by_ref(seed, r);
    $display("ref %0d %0d", r, seed);
    seed = 7;
    by_inout(seed, r);
    $display("inout %0d %0d", r, seed);
    seed = 1;
    aliased(seed, seed);
    $display("aliased %0d", seed);

    seeds[0] = 5;
    seeds[1] = 1;
    seeds[2] = 6;
    r = $random(seeds[1]);
    $display("element %0d %0d %0d %0d", r, seeds[0], seeds[1], seeds[2]);
    h = new;
    h.seed = 1;
    r = $random(h.seed);
    $display("property %0d %0d", r, h.seed);

    wide = 64'h0000_0000_ffff_ffff;
    r = $random(wide);
    $display("time %0d %h", r, wide);
    r48 = 48'hffff_0000_0001;
    r = $random(r48);
    $display("reg48 %0d %h", r, r48);
    s16 = -1;
    r = $random(s16);
    $display("signed16 %0d %0d", r, s16);
    u16 = 1;
    r = $random(u16);
    $display("unsigned16 %0d %0d", r, u16);

    seed = 'x;
    r = $random(seed);
    $display("unknown %0d %0d", r, seed);
    seed = 32'b0000_0000_0000_0000_0000_0000_0000_00z1;
    r = $random(seed);
    $display("partly unknown %0d %0d", r, seed);

    // A distribution and $random share one seed variable in sequence.
    seed = 7;
    other = $dist_uniform(seed, 0, 10);
    r = $dist_uniform(seed, -5, 5);
    $display("shared %0d %0d %0d", other, r, seed);
    $finish;
  end
endmodule
