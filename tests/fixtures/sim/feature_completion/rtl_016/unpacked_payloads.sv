// IEEE 1800-2009 7.3.2, 11.9: unpacked tagged unions with fixed payloads.
// Every member category is constructed, read and written through its own
// type: two-state members convert X/Z to zero (7.2.1), signed members keep
// their sign, and only the active member is accessed.
typedef struct {
    logic [3:0] nibble;
    bit [7:0] code;
    logic signed [5:0] delta;
} record_t;

typedef union tagged {
    void Idle;
    int Count;
    logic [11:0] Raw;
    bit [3:0] Flags;
    shortint signed Offset;
    record_t Record;
    logic [7:0] Bytes [0:2];
    struct packed { logic [1:0] hi; logic [1:0] lo; } Pair;
} payload_t;

typedef union tagged {
    void Empty;
    payload_t Inner;
    logic [3:0] Tail;
} outer_t;

module tb;
    payload_t value, copy;
    outer_t outer;
    logic choose;
    int signed widened;

    initial begin
        value = tagged Count (-3);
        widened = value.Count;
        $display("count=%0d widened=%0d", value.Count, widened);
        value.Count = 32'bx;
        $display("count_two_state=%0d", value.Count);

        value = tagged Raw (12'ha5z);
        $display("raw=%h", value.Raw);
        value.Raw[3:0] = 4'h1;
        $display("raw_part=%h", value.Raw);

        value = tagged Flags (4'b1x0z);
        $display("flags=%b", value.Flags);

        value = tagged Offset (-16'sd300);
        $display("offset=%0d negative=%0d", value.Offset, value.Offset < 0);

        value = tagged Record '{nibble: 4'hx, code: 8'h3c, delta: -6'sd5};
        $display("record nibble=%b code=%h delta=%0d", value.Record.nibble,
                 value.Record.code, value.Record.delta);
        value.Record.code = 8'hzz;
        value.Record.delta = value.Record.delta - 6'sd1;
        $display("record code=%h delta=%0d", value.Record.code,
                 value.Record.delta);

        value = tagged Bytes '{8'h10, 8'h20, 8'hxz};
        $display("bytes=%h %h %h", value.Bytes[0], value.Bytes[1], value.Bytes[2]);
        value.Bytes[2] = 8'h33;
        value.Bytes[0][7:4] = 4'hf;
        $display("bytes=%h %h %h", value.Bytes[0], value.Bytes[1], value.Bytes[2]);

        value = tagged Pair '{hi: 2'b10, lo: 2'b01};
        $display("pair=%b%b", value.Pair.hi, value.Pair.lo);

        copy = value;
        value = tagged Idle;
        $display("copy pair=%b", copy.Pair);

        choose = 1'b1;
        value = choose ? tagged Raw (12'h123) : tagged Idle;
        $display("conditional raw=%h", value.Raw);
        choose = 1'b0;
        value = choose ? tagged Raw (12'h123) : tagged Count (9);
        $display("conditional count=%0d", value.Count);

        outer = tagged Inner (tagged Record '{nibble: 4'h5, code: 8'h66, delta: 6'sd7});
        $display("nested code=%h", outer.Inner.Record.code);
        outer.Inner.Record.nibble = 4'ha;
        $display("nested nibble=%h", outer.Inner.Record.nibble);
        outer = tagged Tail (4'h9);
        $display("tail=%h", outer.Tail);
        $finish(0);
    end
endmodule
