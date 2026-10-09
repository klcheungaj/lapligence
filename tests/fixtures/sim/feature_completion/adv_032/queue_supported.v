module tb;
  integer init, add1, add2, add3, full, length, longest, rm1, rm2, rm3;
  integer job, info, job2, info2, job3, info3, status;
  initial begin
    $q_initialize(1, 1, 2, init);
    $q_add(1, 5, 6, add1);
    $q_add(1, 7, 8, add2);
    $q_add(1, 9, 10, add3);
    full = $q_full(1, status);
    $q_exam(1, 1, length, status);
    $q_exam(1, 3, longest, status);
    $q_remove(1, job, info, rm1);
    $q_remove(1, job2, info2, rm2);
    $q_remove(1, job3, info3, rm3);
    $display("init=%0d add=%0d,%0d,%0d full=%0d len=%0d max=%0d", init, add1, add2, add3, full, length, longest);
    $display("rm=%0d,%0d,%0d job=%0d,%0d info=%0d,%0d", rm1, rm2, rm3, job, job2, info, info2);
    $finish;
  end
endmodule
