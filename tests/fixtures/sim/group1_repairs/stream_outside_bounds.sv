module tb;
    logic [7:0] a[-2:1];
    int lo;
    int hi;
    initial begin
        lo = -3; hi = -1;
        a[-2] = 0; a[-1] = 0; a[0] = 8'h77; a[1] = 8'h88;
        {>>{a with [lo:hi]}} = 24'h112233;
        // An out-of-range unpack reports an error AND writes the valid portion.
        if (a[-2] !== 8'h22 || a[-1] !== 8'h33 || a[0] !== 8'h77 || a[1] !== 8'h88)
            $fatal(1, "out-of-range unpack damaged valid elements");
        $display("in-range portion preserved");
        $finish(0);
    end
endmodule
