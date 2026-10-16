// SIM-099 step 2: shuffle() draws from the calling thread's stream (IEEE
// 1800-2009 7.12.2, 18.14.1; owner decision S28-D5), never from an object's
// stream, and randomize() draws from the object's stream (18.13.3), never
// from the thread's. Each line is a relation that holds for any generator.
`timescale 1ns / 1ns

class pair_c;
  rand bit [15:0] a;
  rand bit [15:0] b;
endclass

class bag_c;
  rand bit [15:0] a;
  int items[$];
  // Runs in the thread that called randomize() (18.6.1).
  function void post_randomize();
    items = '{0, 1, 2, 3, 4, 5, 6, 7, 8, 9};
    items.shuffle();
  endfunction
endclass

module tb;
  int q[$];
  int r[$];

  function automatic void reset_queue(ref int x[$]);
    x = '{0, 1, 2, 3, 4, 5, 6, 7, 8, 9};
  endfunction

  initial begin
    pair_c p = new;
    bag_c bag = new;
    bit [15:0] a1, b1;
    int unsigned x1, x2;
    int post_items[$];

    // Thread shuffles leave an object's stream alone.
    p.srandom(3);
    void'(p.randomize());
    a1 = p.a;
    b1 = p.b;
    p.srandom(3);
    reset_queue(q);
    repeat (3) q.shuffle();
    void'(p.randomize());
    $display("object stream ignores thread shuffles: %0d", p.a == a1 && p.b == b1);

    // An object's randomize() leaves the thread's stream alone, so the
    // shuffle and the next $urandom repeat.
    process::self().srandom(9);
    reset_queue(q);
    q.shuffle();
    x1 = $urandom;
    process::self().srandom(9);
    p.srandom(5);
    void'(p.randomize());
    reset_queue(r);
    r.shuffle();
    x2 = $urandom;
    $display("thread stream ignores object randomize: %0d", q == r && x1 == x2);

    // post_randomize() shuffles from the calling thread: after the same
    // thread seed it matches a direct shuffle, whatever the object's seed.
    process::self().srandom(11);
    bag.srandom(1);
    void'(bag.randomize());
    post_items = bag.items;
    process::self().srandom(11);
    reset_queue(r);
    r.shuffle();
    $display("post_randomize shuffles from the caller: %0d", post_items == r);
    process::self().srandom(11);
    bag.srandom(2);
    void'(bag.randomize());
    $display("object seed does not reach the shuffle: %0d", bag.items == post_items);
    $finish;
  end
endmodule
