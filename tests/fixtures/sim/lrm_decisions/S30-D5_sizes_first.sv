// IEEE 1800-2009 18.5.7.1 L29929-29932: "If an array is constrained by both
// size constraints and iterative constraints, the size constraints are
// solved first, and the iterative constraints next. As a result of this
// implicit ordering between size constraints and iterative constraints, the
// size method shall be treated as a state variable within the foreach block
// of the corresponding array." 18.4 L29370-29371: "If a dynamic array's size
// is not constrained, then the array shall not be resized and all the array
// elements shall be randomized." The text does not say whether a size used
// only by element constraints (a reduction) or only in a disabled block
// counts as constrained.
// Decision S30-D5 (llg reading): array sizes are solved first, over the
// constraints that do not read the elements of an array whose size is random;
// each size is drawn uniformly over the sizes that have a legal completion.
// A size that no enabled constraint mentions keeps the current size.
class sized_c;
  rand int A[];
  constraint c1 { A.size inside {[1:4]}; }
  constraint c2 { foreach (A[i]) A[i] inside {[0:9]}; }
endclass

class kept_c;
  rand byte B[];
  constraint c1 { B.size() == 5; }
endclass

module tb;
  function automatic bit near(int hits, int n, int num, int den);
    longint h, t, a, b, d;
    h = longint'(hits);
    t = longint'(n);
    a = longint'(num);
    b = longint'(den);
    d = h * b - t * a;
    return d * d <= 64'sd25 * t * a * (b - a);
  endfunction

  sized_c s;
  kept_c k;
  int one;

  initial begin
    // Joint uniform solving would give size 1 a weight of 10 in 11110;
    // sizes first give each size 1/4.
    s = new;
    for (int i = 0; i < 1000; i++) begin
      void'(s.randomize());
      if (s.A.size() == 1) one = one + 1;
    end
    $display("size1=%0d", near(one, 1000, 1, 4));
    // With c1 off, no enabled constraint mentions the size.
    k = new;
    k.B = new[2];
    k.c1.constraint_mode(0);
    $display("kept: ok=%0d size=%0d", k.randomize(), k.B.size());
    k.c1.constraint_mode(1);
    $display("sized: ok=%0d size=%0d", k.randomize(), k.B.size());
    $finish;
  end
endmodule
