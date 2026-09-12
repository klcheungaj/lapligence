// llg-test-fixture: tests/fixtures/sim/partial_features/display_extended.sv
module tb;
    reg [3:0] xz;
    reg [7:0] value;

    initial begin
        xz = 4'b1x0z;
        value = 8'h2a;
        $display("hex=%x bin=%b char=%c strength=%v", xz, xz, value, xz);
        $display("upper=%X", value);
        $display("pattern=%p %p", value, xz);
        $write("raw2=%u raw4=%z\n", value, xz);
        $display("time=%0t library=%l", $time);
        $strobe("first=%0d", value);
        value = 8'h2b;
        $strobe("second=%0d", value);
        #1 $finish(0);
    end
endmodule
