// llg-test-fixture: tests/fixtures/sim/string_format/entrypoints.sv
module tb;
    string out;
    string held;
    string fmt;
    reg [127:0] wide;
    reg [7:0] narrow;
    reg [15:0] packed_out;
    integer calls;

    function integer bump;
        begin
            calls = calls + 1;
            bump = calls;
        end
    endfunction

    function automatic string make_string(input integer value);
        begin
            make_string = $sformatf("fn=%0d", value);
        end
    endfunction

    initial begin
        calls = 0;
        $swrite(out, "d=%0d h=%0h b=%0b o=%0o c=%c s=%s r=%0.1f",
                17, 8'haf, 4'b1010, 8'o17, 8'h2a, "ok", 1.5);
        $display("out=<%s>", out);

        $swriteb(out, 4'b1010);
        $display("b=%s", out);
        $swriteo(out, 8'o17);
        $display("o=%s", out);
        $swriteh(out, 8'haf);
        $display("h=%s", out);

        $sformat(out, "prefix:%s:%0d", "x", 3);
        $display("sformat=<%s>", out);
        fmt = "v=%0d";
        $sformat(out, fmt, 7);
        $display("dynamic=<%s>", out);
        out = $sformatf(fmt, 8);
        $display("dynamic-f=<%s>", out);
        $sformat(wide, "AB");
        $display("wide=%h", wide);
        $sformat(wide, "%0.1f", 1.5);
        $display("wide-real=%h", wide);
        $sformat(narrow, "ABCDE");
        $display("narrow=%h", narrow);
        $swrite(packed_out, "ABC");
        $display("packed=%h", packed_out);
        out = make_string(4);
        $display("function=%s", out);

        out = $sformatf("[%s]", $sformatf("%0d", 9));
        $display("nested=%s", out);
        out = $sformatf("");
        $display("empty=<%s>", out);
        out = $sformatf("%0d/%0d", bump(), bump());
        $display("calls=%0d side=%s", calls, out);
        held = $sformatf("hold");
        out = $sformatf("other");
        $display("held=<%s>", held);
        $swrite(out, "left=%0d ", 1, "right=%0h", 8'hf);
        $display("segments=%s", out);
    end
endmodule
