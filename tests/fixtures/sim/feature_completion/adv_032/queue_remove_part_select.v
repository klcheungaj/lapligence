module tb;
  integer status;
  integer info;
  reg [7:0] job;
  initial begin
    $q_initialize(1, 1, 4, status);
    $q_add(1, 5, 6, status);
    $q_remove(1, job[3:0], info, status);
    $finish;
  end
endmodule
