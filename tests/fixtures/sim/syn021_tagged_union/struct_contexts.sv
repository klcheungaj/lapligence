// IEEE 1800-2009 §§7.3.2 and 11.9: the tag and finite payload travel
// together through expressions, calls, selected writes, and NBA publication.
typedef struct packed {
    logic signed [7:0] amount;
    bit [3:0] flag;
} payload_t;

typedef union tagged packed {
    void empty;
    payload_t packet;
    logic [11:0] raw;
} item_t;

typedef union tagged packed {
    logic signed [3:0] signed_nibble;
    logic [3:0] unsigned_nibble;
} nibble_t;

module tb;
    item_t items [0:1];
    item_t copy;
    item_t initialized = tagged raw(12'habc);
    item_t conditional_value;
    item_t copied_out;
    item_t copied_inout;
    item_t same_payload_other_tag;
    nibble_t nibble_value;
    logic signed [15:0] widened;
    integer calls;
    integer selector;
    logic choose;

    function automatic int pick();
        calls = calls + 1;
        return selector;
    endfunction

    function automatic item_t make_packet(input payload_t data);
        return tagged packet(data);
    endfunction

    function automatic item_t pass_value(input item_t data);
        return data;
    endfunction

    function automatic logic [7:0] byte_identity(input logic [7:0] data);
        return data;
    endfunction

    function item_t via_static(input item_t data);
        item_t local_state;
        item_t previous;
        previous = local_state;
        local_state = data;
        return previous;
    endfunction

    task automatic transfer(input item_t source, output item_t destination,
                            inout item_t overwrite, const ref item_t observed);
        destination = source;
        overwrite = observed;
    endtask

    task automatic change_flag(ref item_t data);
        data.packet.flag = 4'h5;
    endtask

    initial begin
        calls = 0;
        selector = 0;
        if (items[0][13:12] !== 2'bxx)
            $fatal(1, "uninitialized tag was not unknown");
        items[0] = tagged empty;
        items[1] = tagged raw(12'habc);
        if ($bits(item_t) !== 14 || items[1].raw !== 12'habc ||
            initialized.raw !== 12'habc)
            $fatal(1, "tag or raw payload layout");

        choose = 1'b1;
        conditional_value = choose
            ? tagged packet('{amount: -8'sd1, flag: 4'h4})
            : tagged raw(12'h555);
        if (conditional_value.packet.amount !== -8'sd1)
            $fatal(1, "selected struct constructor arm");
        choose = 1'b0;
        conditional_value = choose
            ? tagged packet('{amount: -8'sd1, flag: 4'h4})
            : tagged raw(12'h555);
        if (conditional_value.raw !== 12'h555)
            $fatal(1, "selected primitive constructor arm");

        items[0] = make_packet('{amount: -8'sd3, flag: 4'hc});
        copy = pass_value(items[0]);
        same_payload_other_tag = tagged raw(12'hfdc);
        widened = copy.packet.amount;
        if (widened !== -16'sd3 || copy.packet.flag !== 4'hc ||
            copy === same_payload_other_tag ||
            item_t'(copy) !== copy)
            $fatal(1, "struct constructor or value copy");

        copied_out = tagged raw(12'h123);
        copied_inout = tagged raw(12'h456);
        transfer(copy, copied_out, copied_inout, copy);
        if (copied_out.packet.amount !== -8'sd3 ||
            copied_inout.packet.flag !== 4'hc)
            $fatal(1, "formal copy-in/copy-out");
        copied_out = via_static(copy);
        copied_out = via_static(same_payload_other_tag);
        if (copied_out !== copy)
            $fatal(1, "static tagged local did not persist");

        nibble_value = tagged signed_nibble(-4'sd2);
        if (byte_identity({nibble_value.signed_nibble}) !== 8'h0e ||
            byte_identity(signed'({nibble_value.signed_nibble})) !== 8'hfe)
            $fatal(1, "signed tagged concat/cast input");

        change_flag(items[pick()]);
        items[pick()].packet.flag <= 4'h9;
        selector = 1;
        #1;
        if (calls !== 2 || items[0].packet.amount !== -8'sd3 ||
            items[0].packet.flag !== 4'h9 || items[1].raw !== 12'habc)
            $fatal(1, "ref or selected NBA: calls=%0d", calls);

        $display("PASS syn021_struct_contexts");
        $finish;
    end
endmodule
