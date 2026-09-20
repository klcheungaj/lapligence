// llg-test-fixture: tests/fixtures/sim/loops/foreach_mixed_omissions.sv
// IEEE 1800-2009 12.7.3: omitted positions retain dimension numbering.
module tb;
    bit [3:0][2:1] b [5:1][4];
    integer middle;
    integer leading;
    integer trailing;
    integer prefix;
    integer omitted;
    integer order_errors;

    initial begin
        middle = 0;
        order_errors = 0;
        foreach (b[q,r,,s]) begin
            if (q != 5 - middle / 8 || r != (middle / 2) % 4 || s != 2 - middle % 2)
                order_errors++;
            middle++;
        end
        leading = 0;
        foreach (b[,r,,s]) begin
            if (r != leading / 2 || s != 2 - leading % 2) order_errors++;
            leading++;
        end
        trailing = 0;
        foreach (b[q,,,]) begin
            if (q != 5 - trailing) order_errors++;
            trailing++;
        end
        prefix = 0;
        foreach (b[q]) begin
            if (q != 5 - prefix) order_errors++;
            prefix++;
        end
        omitted = 17;
        foreach (b[,,,]) omitted++;
        foreach (b[]) omitted++;
        $display("middle=%0d leading=%0d trailing=%0d prefix=%0d omitted=%0d errors=%0d",
                 middle, leading, trailing, prefix, omitted, order_errors);
        $finish(0);
    end
endmodule
