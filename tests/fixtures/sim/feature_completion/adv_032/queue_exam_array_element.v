module tb;
  integer status;
  integer stats [0:3];
  initial begin
    $q_initialize(1, 1, 4, status);
    $q_exam(1, 1, stats[2], status);
    $finish;
  end
endmodule
