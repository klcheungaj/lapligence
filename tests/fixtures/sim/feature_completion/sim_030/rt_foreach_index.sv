// SIM-030: an index outside the array is not eliminated (18.5.7.1, S30-D11).
class sorted_c;
  rand int A[];
  constraint c1 { A.size() == 3; }
  constraint c2 { foreach (A[k]) A[k + 1] > A[k]; }
endclass

module tb;
  sorted_c s;
  int ok;
  initial begin
    s = new;
    ok = s.randomize();
    $display("ok=%0d size=%0d", ok, s.A.size());
    s.c2.constraint_mode(0);
    ok = s.randomize();
    $display("ok=%0d size=%0d", ok, s.A.size());
    $finish;
  end
endmodule
