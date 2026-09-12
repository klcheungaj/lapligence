// IEEE 1800-2009 7.5 and 7.6: nested dynamic arrays copy owned child values,
// preserve prefixes during resize, and release child storage on delete.
module tb;
    int source[];
    int nested[][];
    int copy[][];

    initial begin
        source = new[2];
        source[0] = 10;
        source[1] = 20;
        nested = new[2];
        nested[0] = source;
        nested[0][1] = 21;

        copy = nested;
        nested[0][0] = 11;
        if (copy[0][0] != 10 || copy[0][1] != 21) begin
            $display("FAIL nested_dynamic_arrays deep_copy");
            $finish;
        end

        nested = new[3](nested);
        if (nested.size() !== 3 || nested[0][0] != 11 ||
            nested[0][1] != 21) begin
            $display("FAIL nested_dynamic_arrays resize");
            $finish;
        end

        nested.delete();
        source.delete();
        if (nested.size() !== 0 || source.size() !== 0) begin
            $display("FAIL nested_dynamic_arrays delete");
            $finish;
        end
        $display("PASS nested_dynamic_arrays");
        $finish;
    end
endmodule
