// SIM-099 step 2: deterministic random replay across every random source of
// the common stack (IEEE 1800-2009 18.13-18.17): $urandom, $urandom_range,
// $random, object randomize() with constraints and dist, std::randomize,
// shuffle, randcase and randsequence. Restoring a thread's or an object's
// saved state replays its draws exactly (18.13.4, 18.13.5). A randc
// variable's cycle history is not generator state (18.4.2), so randc takes
// part only in the checksum of a fresh object. The values
// themselves depend on the generator, so the suite requires the checksum
// line to be identical in every optimizer mode, value backend, repeated run
// and collector setting.
`timescale 1ns / 1ns

class packet_c;
  rand bit [7:0] addr;
  rand bit [3:0] kind;
  constraint c_addr {addr inside {[8'h10 : 8'h7f]};}
  constraint c_kind {kind dist {0 := 1, [1 : 3] :/ 2, [4 : 15] :/ 1};}
endclass

class cyclic_c;
  randc bit [2:0] tag;
endclass

module tb;
  int unsigned sum;
  int unsigned trace[$];
  int signed legacy_seed;

  function automatic void mix(int unsigned value);
    sum = (sum * 32'd16777619) ^ value;
    trace.push_back(value);
  endfunction

  // One round of every source, drawn from the calling thread and from `p`.
  task automatic round(packet_c p);
    int q[$] = '{1, 2, 3, 4, 5, 6, 7, 8};
    int unsigned x;
    bit [5:0] y;
    mix($urandom);
    mix($urandom_range(1000, 10));
    mix($random(legacy_seed));
    void'(p.randomize());
    mix({p.addr, p.kind});
    void'(p.randomize() with { addr < 8'h20; });
    mix({p.addr, p.kind});
    void'(std::randomize(x, y) with { y > 6'd40; });
    mix(x ^ 32'(y));
    q.shuffle();
    foreach (q[i]) mix(q[i] << i);
    randcase
      1: mix(32'd101);
      2: mix(32'd202);
      5: mix(32'd505);
    endcase
    randsequence (top)
      top : first second;
      first : a := 1 | b := 3;
      second : repeat ($urandom_range(3, 1)) c;
      a : { mix(32'd7); };
      b : { mix(32'd11); };
      c : { mix(32'd13); };
    endsequence
  endtask

  initial begin
    packet_c p = new;
    cyclic_c c = new;
    bit [7:0] seen;
    string thread_state;
    string object_state;
    int unsigned first_sum;
    int first_size;
    int unsigned replay[$];
    int replayed_seed;
    legacy_seed = 5;
    process::self().srandom(17);
    p.srandom(23);
    // Save both states, run two rounds, restore, run them again.
    thread_state = process::self().get_randstate();
    object_state = p.get_randstate();
    replayed_seed = legacy_seed;
    sum = 0;
    round(p);
    round(p);
    first_sum = sum;
    first_size = trace.size();
    replay = trace;
    trace.delete();
    process::self().set_randstate(thread_state);
    p.set_randstate(object_state);
    legacy_seed = replayed_seed;
    sum = 0;
    round(p);
    round(p);
    $display("replay equal: %0d", trace == replay && sum == first_sum && first_size > 0);
    // A third round continues the streams, so it differs from the first.
    trace.delete();
    round(p);
    replay = replay[0:trace.size() - 1];
    $display("continuation differs: %0d", trace != replay);
    // One full randc cycle visits every value of the 3-bit range once.
    seen = 0;
    for (int i = 0; i < 8; i++) begin
      void'(c.randomize());
      seen[c.tag] = 1'b1;
      mix(32'(c.tag));
    end
    $display("randc cycle complete: %0d", seen == 8'hff);
    $display("checksum %08h %08h (%0d draws)", first_sum, sum, first_size);
    $finish;
  end
endmodule
