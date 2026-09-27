// llg-test-fixture: tests/fixtures/sim/memory_editions/signed_address.sv
module tb;
  reg [7:0] ascending [-129:15];
  reg [7:0] descending [15:-129];
  integer i;
  initial begin
    for (i = -129; i <= 15; i = i + 1) begin
      ascending[i] = 0;
      descending[i] = 0;
    end
    $readmemh("signed.mem", ascending, -129, 15);
    $readmemh("signed.mem", descending, 15, -129);
    $display("asc=%h,%h,%h,%h,%h,%h desc=%h,%h,%h,%h,%h,%h",
             ascending[-7], ascending[-8], ascending[-9], ascending[-15],
             ascending[-128], ascending[-129],
             descending[-7], descending[-8], descending[-9], descending[-15],
             descending[-128], descending[-129]);
  end
endmodule
