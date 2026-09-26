// llg-test-fixture: tests/fixtures/sim/syn030_memory_views/selected_element_types.sv
module tb;
  typedef struct packed {
    logic [3:0] high;
    logic [3:0] low;
  } word_t;
  word_t words [0:1][0:1];
  bit [3:0] bits [0:1][0:1];
  int row;
  initial begin
    foreach (words[i,j]) words[i][j] = 8'hee;
    foreach (bits[i,j]) bits[i][j] = 4'hf;
    row = 1;
    $readmemh("struct.mem", words[row]);
    $readmemb("bits.mem", bits[row]);
    $display("struct=%h,%h,%h bits=%b,%b,%b",
             words[1][0], words[1][1], words[0][0],
             bits[1][0], bits[1][1], bits[0][0]);
  end
endmodule
