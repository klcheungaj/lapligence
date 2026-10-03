// IEEE 1800-2009 7.3.2, 11.9, 13.5, 23.3: unpacked tagged unions with fixed
// payloads cross value, output, inout and ref formals, function results,
// ports, static and automatic locals, unpacked arrays and records. Selected
// active members are legal actuals for formals of the member type.
typedef struct { logic [3:0] lo; bit [7:0] code; } record_t;

typedef union tagged {
    int Count;
    record_t Record;
    logic [7:0] Bytes [0:1];
} item_t;

module relay(input item_t source, output item_t sink);
    always_comb sink = source;
endmodule

module tb;
    item_t value, other, cells [0:2], driven, relayed;
    struct { item_t inner; int tag_count; } holder;
    record_t plain;
    int calls;

    relay link(.source(driven), .sink(relayed));

    function automatic item_t make_count(input int amount);
        item_t local_value;
        local_value = tagged Count (amount);
        return local_value;
    endfunction

    function automatic int count_of(input item_t data);
        return data.Count;
    endfunction

    function item_t remember(input item_t data);
        item_t previous;
        item_t result;
        result = previous;
        previous = data;
        return result;
    endfunction

    function automatic record_t bump_record(input record_t data);
        data.code = data.code + 8'h01;
        return data;
    endfunction

    task automatic fill(output item_t data, inout item_t through);
        data = tagged Bytes '{8'h0a, 8'h0b};
        through.Record.lo = through.Record.lo + 4'h1;
    endtask

    task automatic touch(ref item_t data, const ref item_t seen);
        data.Record.code = seen.Record.code;
    endtask

    task automatic set_byte(ref logic [7:0] target, input logic [7:0] data);
        target = data;
    endtask

    task automatic copy_record(output record_t target, input record_t data);
        target = data;
    endtask

    function automatic int pick(input int index);
        calls++;
        return index;
    endfunction

    initial begin
        calls = 0;
        value = make_count(-4);
        $display("count_of=%0d", count_of(value));
        void'(remember(tagged Count (1)));
        other = remember(tagged Count (2));
        $display("static previous=%0d", other.Count);

        value = tagged Record '{lo: 4'h1, code: 8'h20};
        other = tagged Record '{lo: 4'h0, code: 8'h99};
        fill(cells[0], value);
        touch(value, other);
        $display("filled=%h %h lo=%h code=%h", cells[0].Bytes[0], cells[0].Bytes[1],
                 value.Record.lo, value.Record.code);

        plain = bump_record(value.Record);
        $display("selected input=%h", plain.code);
        copy_record(value.Record, '{lo: 4'h7, code: 8'h70});
        $display("selected output lo=%h code=%h", value.Record.lo, value.Record.code);
        set_byte(cells[0].Bytes[pick(1)], 8'hbb);
        $display("selected ref=%h calls=%0d", cells[0].Bytes[1], calls);

        cells[pick(2)] = tagged Count (12);
        cells[pick(2)].Count <= 13;
        holder.inner = tagged Record '{lo: 4'h2, code: 8'h22};
        holder.inner.Record.code = holder.inner.Record.code + 8'h01;
        holder.tag_count = 1;
        driven = tagged Bytes '{8'hc0, 8'hc1};
        #1;
        $display("cells2=%0d calls=%0d holder=%h", cells[2].Count, calls,
                 holder.inner.Record.code);
        $display("relayed=%h %h", relayed.Bytes[0], relayed.Bytes[1]);
        driven.Bytes[1] = 8'hc2;
        #1;
        $display("relayed=%h", relayed.Bytes[1]);
        $finish(0);
    end
endmodule
