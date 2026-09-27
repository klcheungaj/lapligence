module tb;
  typedef enum logic [7:0] {ZERO = 8'h00, ONE = 8'h01} state_t;
  state_t mem [0:9];
  initial begin
    for (integer i = 0; i < 10; i++) mem[i] = ZERO;
    $readmemh("q02_hex_x.mem", mem, 0, 0);
    $readmemh("q02_hex_z.mem", mem, 1, 1);
    $readmemh("q02_hex_1x.mem", mem, 2, 2);
    $readmemh("q02_hex_x1.mem", mem, 3, 3);
    $readmemh("q02_hex_zX.mem", mem, 4, 4);
    $readmemb("q02_bin_x.mem", mem, 5, 5);
    $readmemb("q02_bin_z.mem", mem, 6, 6);
    $readmemb("q02_bin_1x.mem", mem, 7, 7);
    $readmemb("q02_bin_x1.mem", mem, 8, 8);
    $readmemb("q02_bin_zX.mem", mem, 9, 9);
    $display("Q02.enum_tokens hex=%b,%b,%b,%b,%b bin=%b,%b,%b,%b,%b", mem[0], mem[1], mem[2], mem[3], mem[4], mem[5], mem[6], mem[7], mem[8], mem[9]);
  end
endmodule
