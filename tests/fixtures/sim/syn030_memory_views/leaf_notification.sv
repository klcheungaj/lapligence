// llg-test-fixture: tests/fixtures/sim/syn030_memory_views/leaf_notification.sv
module tb;
  logic [7:0] mem [0:1][0:1];
  int changed_wakes, unchanged_wakes, other_wakes;
  always @(mem[1][0]) changed_wakes++;
  always @(mem[1][1]) unchanged_wakes++;
  always @(mem[0][0]) other_wakes++;
  initial begin
    mem[0][0] = 8'h33; mem[0][1] = 8'h44;
    mem[1][0] = 8'h11; mem[1][1] = 8'h22;
    #1;
    changed_wakes = 0; unchanged_wakes = 0; other_wakes = 0;
    #1;
    $readmemh("notify.mem", mem[1]);
    #1;
    $display("values=%h,%h,%h wakes=%0d,%0d,%0d",
             mem[1][0], mem[1][1], mem[0][0],
             changed_wakes, unchanged_wakes, other_wakes);
    $finish;
  end
endmodule
