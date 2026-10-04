// llg-test-fixture: tests/fixtures/sim/feature_completion/rtl_019/legacy_forms_2001.v
// IEEE 1364-2001 19.7, 12.1.3, 10.2-10.3, 17.2: the nearest legal neighbours of
// the strict 2001 gates, including memory-storage arguments of $readmemh/$fread.
`line 1 "legacy_mapped.v" 0
module leaf (a, y);
  parameter W = 4;
  input [W-1:0] a;
  output [2*W-1:0] y;
  assign y = {a, a};
endmodule
module tb;
  reg [3:0] a;
  wire [7:0] y;
  reg [7:0] mem [0:3];
  reg [7:0] copy [0:3];
  reg [7:0] raw [0:3];
  reg flag = 1'b1;
  integer fd, i;
  genvar g;
  leaf #(4) u (.a(a), .y(y));
  generate
    for (g = 0; g < 2; g = g + 1) begin : gen
      wire [3:0] w = a + g;
    end
  endgenerate
  task add;
    input [7:0] x;
    output [7:0] z;
    begin : body
      reg [7:0] tmp;
      tmp = x + 1;
      z = tmp;
    end
  endtask
  function [7:0] inc;
    input [7:0] x;
    inc = x + 1;
  endfunction
  initial begin : main
    reg [7:0] t;
    a = 4'h3;
    mem[0] = 8'h12;
    mem[1] = 8'h34;
    mem[2] = 8'h56;
    mem[3] = 8'h78;
    fd = $fopen("rtl019_legacy.hex", "w");
    for (i = 0; i < 4; i = i + 1)
      $fdisplay(fd, "%h", mem[i]);
    $fclose(fd);
    $readmemh("rtl019_legacy.hex", copy);
    fd = $fopen("rtl019_legacy.hex", "r");
    i = $fread(raw, fd);
    $fclose(fd);
    add(copy[1], t);
    #1 $display("y=%h gen1=%h t=%h inc=%h flag=%b copy3=%h", y, gen[1].w, t, inc(copy[2]), flag, copy[3]);
    $display("raw=%h %h %h %h read=%0d", raw[0], raw[1], raw[2], raw[3], i);
    $finish;
  end
endmodule
