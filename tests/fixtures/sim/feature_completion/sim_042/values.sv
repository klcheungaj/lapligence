// llg-test-fixture: SIM-042 value access. values.c ($formats, $schedule,
// $force_some, $release_some, $late) reads every Table 38-3 format with exact
// X/Z, and writes through vpi_put_value with every delay mode, scheduled-event
// handles, cancellation, force and release. Expected output: values.out,
// derived in readme.md.
`timescale 1ns/1ns
module tb;
    logic [11:0] v = 12'b1x0z_0101_zzzz;
    logic [5:0] xs = 6'bxxxxxx;
    logic [3:0] zs = 4'bzzzz;
    logic [3:0] pz = 4'b10z1;
    logic signed [15:0] s = -16'sd300;
    logic [71:0] wide = {8'h81, 64'hx};
    logic [31:0] str = "Hi!";
    real r = -2.5;
    logic [3:0] d = 4'h0;
    wire [3:0] n;
    logic [7:0] f = 8'h0f;
    assign n = d;
    always @(d) $display("%0t d=%h", $time, d);
    initial begin
        $formats;
        $schedule;
        #10 $late;
        $force_some;
        #1 $display("%0t forced f=%h n=%h", $time, f, n);
        f = 8'h11;
        d = 4'h9;
        #1 $display("%0t written f=%h n=%h d=%h", $time, f, n, d);
        $release_some;
        $display("%0t released f=%h n=%h", $time, f, n);
        f = 8'h22;
        #1 $display("%0t final f=%h n=%h", $time, f, n);
        $finish;
    end
endmodule
