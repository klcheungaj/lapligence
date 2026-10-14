module tb;
    import "DPI-C" function void dpi_crash();

    initial begin
        dpi_crash();
        $finish;
    end
endmodule
