// IEEE 1800-2009 7.12.2: mapped ordering preserves whole unpacked elements.
module tb;
    typedef struct { logic signed [7:0] key; bit [7:0] id; } record_t;
    record_t values [3:0];

    function automatic int sort_in_function(ref record_t items [3:0]);
        items.sort() with (item.key);
        return items[3].key;
    endfunction

    initial begin
        values[3] = '{-8'sd2, 8'd10};
        values[2] = '{8'sd1, 8'd11};
        values[1] = '{-8'sd2, 8'd12};
        values[0] = '{8'sd0, 8'd13};
        if (sort_in_function(values) != -2 ||
            values[3].key !== -8'sd2 || values[2].key !== -8'sd2 ||
            values[1].key !== 8'sd0 || values[0].key !== 8'sd1 ||
            values[1].id !== 8'd13 || values[0].id !== 8'd11 ||
            !((values[3].id === 8'd10 && values[2].id === 8'd12) ||
              (values[3].id === 8'd12 && values[2].id === 8'd10)))
            $fatal(1, "ascending mapped records");

        values.rsort() with (item.key);
        if (values[3].key !== 8'sd1 || values[2].key !== 8'sd0 ||
            values[1].key !== -8'sd2 || values[0].key !== -8'sd2 ||
            values[3].id !== 8'd11 || values[2].id !== 8'd13 ||
            !((values[1].id === 8'd10 && values[0].id === 8'd12) ||
              (values[1].id === 8'd12 && values[0].id === 8'd10)))
            $fatal(1, "descending mapped records");
        $display("PASS syn028_unpacked_record_maps");
        $finish(0);
    end
endmodule
