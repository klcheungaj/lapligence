// SIM-028 A02: shuffle() (IEEE 1800-2009 7.12.2) is a permutation and draws
// from the calling thread's random stream (18.14.1 thread stability; the
// owner is an llg choice, S28-D5). Only relations are checked.
class holder_c;
  int v;
  function new(int value);
    v = value;
  endfunction
  // A method body still draws from the calling thread, not the object.
  function void shuffle_into(ref int q[$]);
    q.shuffle();
  endfunction
endclass

module tb;
  int q[$], r[$], sorted[$];
  int dyn[];
  int fixed[6];
  string words[$];
  real reals[$];
  holder_c handles[$];
  holder_c h;
  int unsigned x, y, after_plain;
  int sum;

  function automatic bit is_permutation(int got[$], int n);
    int seen[$];
    seen = got;
    seen.sort();
    if (seen.size() != n) return 0;
    foreach (seen[i]) if (seen[i] != i) return 0;
    return 1;
  endfunction

  initial begin
    q = '{0, 1, 2, 3, 4, 5, 6, 7, 8, 9};
    q.shuffle();
    $display("queue permutation %0d", is_permutation(q, 10));
    dyn = new[7];
    foreach (dyn[i]) dyn[i] = i;
    dyn.shuffle();
    r = dyn;
    $display("dynamic permutation %0d", is_permutation(r, 7));
    foreach (fixed[i]) fixed[i] = i;
    fixed.shuffle();
    r.delete();
    foreach (fixed[i]) r.push_back(fixed[i]);
    $display("fixed permutation %0d", is_permutation(r, 6));
    words = '{"a", "b", "c", "d"};
    words.shuffle();
    words.sort();
    $display("strings %s%s%s%s", words[0], words[1], words[2], words[3]);
    reals = '{0.5, 1.5, 2.5};
    reals.shuffle();
    reals.sort();
    $display("reals %0.1f %0.1f %0.1f", reals[0], reals[1], reals[2]);
    for (int i = 0; i < 5; i++) begin
      h = new(i);
      handles.push_back(h);
    end
    handles.shuffle();
    sum = 0;
    foreach (handles[i]) sum += handles[i].v * 10 ** handles[i].v;
    $display("handles %0d", sum);
    r.delete();
    r.shuffle();
    $display("empty %0d", r.size());
    r = '{42};
    r.shuffle();
    $display("single %0d", r[0]);

    // Reseeding the calling thread reproduces the permutation and the
    // thread's following draws.
    process::self().srandom(9);
    q = '{0, 1, 2, 3, 4, 5, 6, 7, 8, 9};
    q.shuffle();
    x = $urandom;
    process::self().srandom(9);
    r = '{0, 1, 2, 3, 4, 5, 6, 7, 8, 9};
    r.shuffle();
    y = $urandom;
    $display("thread seed reproduces %0d %0d", q == r, x == y);
    process::self().srandom(9);
    after_plain = $urandom;
    $display("shuffle draws from the thread %0d", after_plain != x);

    // A shuffle in a child thread does not perturb the parent.
    process::self().srandom(9);
    fork
      q.shuffle();
    join
    x = $urandom;
    process::self().srandom(9);
    fork
      sum = 0;
    join
    y = $urandom;
    $display("child shuffle isolated %0d", x == y);

    // Inside a method the object's own stream is not used.
    h.srandom(1);
    process::self().srandom(9);
    q = '{0, 1, 2, 3, 4, 5, 6, 7, 8, 9};
    h.shuffle_into(q);
    h.srandom(2);
    process::self().srandom(9);
    r = '{0, 1, 2, 3, 4, 5, 6, 7, 8, 9};
    h.shuffle_into(r);
    $display("method uses thread %0d", q == r);
    $finish;
  end
endmodule
