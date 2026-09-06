// IEEE 1800-2009 7.2 and 7.2.2: unpacked structures have named typed
// members, support whole-structure assignment, and copy by value.
module tb;
    typedef struct {
        integer identifier;
        logic [7:0] code;
        bit enabled;
    } record_t;

    record_t original;
    record_t copy;

    initial begin
        if (original.identifier !== 32'hxxxxxxxx ||
            original.code !== 8'hxx || original.enabled !== 1'b0) begin
            $display("FAIL unpacked_struct defaults");
            $finish;
        end

        original = '{identifier: 17, code: 8'ha5, enabled: 1'b1};
        copy = original;
        original.identifier = 23;
        original.code = 8'h5a;
        original.enabled = 1'b0;

        if (copy.identifier !== 17 || copy.code !== 8'ha5 ||
            copy.enabled !== 1'b1) begin
            $display("FAIL unpacked_struct whole_copy");
            $finish;
        end
        if (original.identifier !== 23 || original.code !== 8'h5a ||
            original.enabled !== 1'b0) begin
            $display("FAIL unpacked_struct member_write");
            $finish;
        end

        $display("PASS unpacked_struct");
        $finish;
    end
endmodule
