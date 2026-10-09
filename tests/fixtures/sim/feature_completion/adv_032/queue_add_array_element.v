module tb;
  integer status;
  integer ids [0:3];
  initial begin
    $q_initialize(1, 1, 4, status);
    $q_add(1, 5, 6, ids[1]);
    $finish;
  end
endmodule
