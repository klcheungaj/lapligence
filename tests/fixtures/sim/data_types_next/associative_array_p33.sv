// IEEE 1800-2009 7.8 and 7.9: associative arrays preserve declared integral
// ordering, explicit missing-entry defaults, copy independence, wildcard-key
// canonicalization, and all legal traversal status/update rules.
module tb;
    typedef logic signed [7:0] key_t;
    logic [15:0] signed_values[key_t];
    logic [15:0] copied_values[key_t];
    logic [7:0] wildcard_values[*];
    key_t key;
    integer status;

    initial begin
        signed_values = '{default:16'h0055, -2:16'h0022, 3:16'h0033};
        if (signed_values.num() !== 2 || signed_values.exists(4) !== 0 ||
            signed_values[4] !== 16'h0055 || signed_values[-2] !== 16'h0022) begin
            $display("FAIL associative_array_p33 defaults");
            $finish;
        end

        status = signed_values.first(key);
        if (status !== 1 || key !== -2) begin
            $display("FAIL associative_array_p33 first");
            $finish;
        end
        status = signed_values.next(key);
        if (status !== 1 || key !== 3) begin
            $display("FAIL associative_array_p33 next");
            $finish;
        end
        status = signed_values.next(key);
        if (status !== 0 || key !== 3) begin
            $display("FAIL associative_array_p33 next_end");
            $finish;
        end
        status = signed_values.last(key);
        if (status !== 1 || key !== 3) begin
            $display("FAIL associative_array_p33 last");
            $finish;
        end
        status = signed_values.prev(key);
        if (status !== 1 || key !== -2) begin
            $display("FAIL associative_array_p33 prev");
            $finish;
        end
        status = signed_values.prev(key);
        if (status !== 0 || key !== -2) begin
            $display("FAIL associative_array_p33 prev_end");
            $finish;
        end

        copied_values = signed_values;
        signed_values.delete(3);
        if (signed_values.num() !== 1 || copied_values.num() !== 2 ||
            copied_values[3] !== 16'h0033) begin
            $display("FAIL associative_array_p33 copy");
            $finish;
        end
        signed_values = '{default:16'h0099, 5:16'h0055};
        if (signed_values.num() !== 1 || signed_values[-2] !== 16'h0099 ||
            signed_values[5] !== 16'h0055) begin
            $display("FAIL associative_array_p33 replace");
            $finish;
        end

        wildcard_values = '{default:8'h07};
        wildcard_values[8'hff] = 8'h11;
        wildcard_values[16'h00ff] = 8'h22;
        if (wildcard_values.num() !== 1 || wildcard_values[8'hff] !== 8'h22 ||
            wildcard_values[8'h01] !== 8'h07) begin
            $display("FAIL associative_array_p33 wildcard");
            $finish;
        end

        $display("PASS associative_array_p33");
        $finish;
    end
endmodule
