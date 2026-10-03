// IEEE 1800-2009 §10.9: one evaluated default value and independent queued fill.
module tb;
    logic [16:0] values [0:16777215];
    integer input_value = 17'h12345;
    initial begin
        values = '{default: input_value};
        if (values[0] !== 17'h12345 || values[65536] !== 17'h12345 || values[16777215] !== 17'h12345) $fatal;
        values[3] = 0;
        values <= '{default: input_value};
        input_value = 0;
        #1;
        if (values[3] !== 17'h12345 || values[16777215] !== 17'h12345) $fatal;
        $display("PASS rtl002 default pattern");
        $finish(0);
    end
endmodule
