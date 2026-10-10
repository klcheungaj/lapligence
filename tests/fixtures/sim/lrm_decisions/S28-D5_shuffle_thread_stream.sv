// IEEE 1800-2009 7.12.2 L9366: "shuffle() randomizes the order of the
// elements in the array." 18.14.1 L31152-31153: "Thread stability. Each
// thread has an independent RNG for all randomization system calls invoked
// from that thread."
// Decision (llg choice; the text does not name the RNG shuffle uses): shuffle
// draws from the calling thread's RNG, also inside a class method, so
// reseeding the thread reproduces the order and the thread's later values.
class holder_c;
  function void mix(ref int q[$]);
    q.shuffle();
  endfunction
endclass

module tb;
  int a[$], b[$];
  int unsigned x, y, plain;
  holder_c h;
  initial begin
    h = new;
    process::self().srandom(9);
    a = '{0, 1, 2, 3, 4, 5, 6, 7, 8, 9};
    a.shuffle();
    x = $urandom;
    process::self().srandom(9);
    b = '{0, 1, 2, 3, 4, 5, 6, 7, 8, 9};
    b.shuffle();
    y = $urandom;
    $display("same order %0d same next value %0d", a == b, x == y);
    process::self().srandom(9);
    plain = $urandom;
    $display("shuffle drew values %0d", plain != x);
    h.srandom(1);
    process::self().srandom(9);
    b = '{0, 1, 2, 3, 4, 5, 6, 7, 8, 9};
    h.mix(b);
    $display("method uses the thread %0d", a == b);
    a.sort();
    b = '{0, 1, 2, 3, 4, 5, 6, 7, 8, 9};
    $display("permutation %0d", a == b);
    $finish;
  end
endmodule
