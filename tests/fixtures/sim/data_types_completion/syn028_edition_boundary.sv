// IEEE 1800-2009 7.12.2 adds fixed-array ordering to Verilog-2001 arrays.
module tb;
    reg [7:0] values [0:1];
    initial begin
        values[0] = 8'd2;
        values[1] = 8'd1;
        values.sort();
        if (values[0] !== 8'd1 || values[1] !== 8'd2)
            $fatal(1, "sort order");
        values.rsort();
        if (values[0] !== 8'd2 || values[1] !== 8'd1)
            $fatal(1, "rsort order");
        $display("PASS syn028_edition_boundary");
        $finish(0);
    end
endmodule
