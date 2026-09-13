module tb;
    string s,t;
    chandle h,k;
    logic [7:0] byte_value;
    initial begin
        if (s != "" || t != "" || h != null || k != null) $display("FAIL defaults");
        s="Az09"; t=s; s.putc(0,8'h62);
        if (s != "bz09" || t != "Az09") $display("FAIL string value copy");
        t=s.toupper();
        if (t != "BZ09" || s != "bz09" || t.substr(1,2) != "Z0") $display("FAIL string methods");
        byte_value=s.getc(0);
        if (byte_value !== 8'h62 || s.getc(99) !== 0) $display("FAIL character access");
        s="125tail";
        if (s.atoi() !== 125) $display("FAIL numeric prefix");
        s="-125tail";
        if (s.atoi() !== 0) $display("FAIL numeric sign");
        s.itoa(-125); t=s;
        if (s != "-125" || t.atoi() !== 0) $display("FAIL negative integer string");
        h=null; k=h;
        if (h != k || h != null) $display("FAIL chandle copy");
        $display("PASS object types"); $finish(0);
    end
endmodule
