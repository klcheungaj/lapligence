module tb;
    function automatic void bad(const ref string source);
        source.putc(0, 8'h41);
    endfunction

    initial begin
        string value;
        value = "x";
        bad(value);
        $finish(0);
    end
endmodule
