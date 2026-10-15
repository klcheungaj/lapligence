// IEEE 1800-2009 18.5.7.1 L29926-29927: "Index expressions can include loop
// variables, constants, and state variables. Invalid or out or bound array
// indices are not automatically eliminated; users must explicitly exclude
// these indices using predicates." L29919-29921: "predicate expressions
// involving only constants, state variables, object handle comparisons, loop
// variables, or the size of the array being iterated behave as guards
// against the creation of constraints". The text does not say what an index
// that is a random variable selects outside the array.
// Decision S30-D11 (llg reading): a constant, state or loop index outside the
// array is an evaluation error: randomize() returns 0, the variables keep
// their values, `select index out of range in a constraint` is reported and
// the run ends with exit status 1. A guard that is false removes the
// constraint first. A random index selects only existing elements: an
// assignment whose index is outside the array is not a solution (as S30-D4).
class sorted_c;
  rand int A[];
  rand bit [2:0] k;
  rand bit [3:0] B[5];
  constraint c1 { A.size() == 3; }
  constraint guarded { foreach (A[i]) (i < A.size() - 1) -> A[i + 1] > A[i]; }
  constraint picked { B[k] == 4'd9; }
  constraint unguarded { foreach (A[i]) A[i + 1] > A[i]; }
endclass

module tb;
  sorted_c s;
  int outside;
  initial begin
    s = new;
    s.unguarded.constraint_mode(0);
    for (int i = 0; i < 100; i++) begin
      void'(s.randomize());
      if (s.k > 3'd4) outside = outside + 1;
    end
    $display("guarded: sorted=%0d outside=%0d", s.A[1] > s.A[0] && s.A[2] > s.A[1], outside);
    s.unguarded.constraint_mode(1);
    $display("unguarded: ok=%0d size=%0d", s.randomize(), s.A.size());
    $finish;
  end
endmodule
