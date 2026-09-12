// llg-test-fixture: tests/fixtures/sim/partial_features/display_formatting.sv
module tb;
    reg [7:0] value;
    real amount;
    string text;

    initial begin
        value = 8'haf;
        amount = -2.5;
        text = "sv";
        $display("d=[%06d] h=[%06h] b=[%-8b] o=[%8o] r=[%.2f] s=[%8s] m=%m %%",
                 value, value, value, value, amount, text);
        $write("tail");
        $strobe("strobe %s %.1f %0d", text, amount, value);
        #1 $finish(0);
    end
endmodule
