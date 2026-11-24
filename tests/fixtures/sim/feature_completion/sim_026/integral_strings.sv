// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_026/integral_strings.sv
// String arguments held in integral values (IEEE 1800-2009 21.3.1, 21.6).
module tb;
  reg [8*16-1:0] r;
  logic [63:0] mode, text;
  integer i, c, fd;

  initial begin
    r = "integral.dat";
    mode = "w";
    fd = $fopen(r, mode);
    $display("write %0d", fd != 0);
    $fdisplay(fd, "a5");
    $fclose(fd);
    mode = {"r", 56'h0};
    fd = $fopen({"integral", ".dat"}, mode);
    c = $fscanf(fd, "%h", i);
    $display("read %0d %h", c, i);
    $fclose(fd);

    r = "ABC";
    text = "AB";
    $display("test %0d %0d %0d", $test$plusargs(r), $test$plusargs(text),
             $test$plusargs({"A", "C"}));
    r = "V=%d";
    c = $value$plusargs(r, i);
    $display("value %0d %0d", c, i);
    $finish(0);
  end
endmodule
