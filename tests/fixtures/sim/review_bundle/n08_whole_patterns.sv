// IEEE 1800-2009 12.6: wildcard/binding patterns preserve the entire type.
module tb;
    typedef struct packed { logic [3:0] hi, lo; } packed_t;
    typedef struct { int key; logic [7:0] lanes[2]; } record_t;
    typedef union tagged packed { void invalid; logic [7:0] valid; } choice_t;
    packed_t packet;
    record_t record_value;
    choice_t choice;
    typedef logic [7:0] row_t[2];
    int calls, result;

    function automatic packed_t sample();
        calls++;
        return packet;
    endfunction
    function automatic record_t sample_record();
        calls++;
        return record_value;
    endfunction

    function automatic int bind_local(input record_t input_record);
        row_t local_row;
        local_row = input_record.lanes;
        if (local_row matches .row &&& row[0] == 8'h21)
            return row[1];
        return 0;
    endfunction

    initial begin
        calls = 0;
        packet = 8'ha5;
        if (sample() matches .whole &&& whole.lo == 5) begin
            packet = 0;
            result = whole.hi;
        end else $fatal(1, "whole packed binding");
        if (result != 10 || calls != 1) $fatal(1, "packed source snapshot");
        packet = 'x;
        if (sample() matches .*) result = 1;
        else $fatal(1, "wildcard must accept unknown bits");
        if (calls != 2) $fatal(1, "wildcard source evaluation");
        if (1'b0 &&& sample() matches .suppressed) $fatal(1, "false prefix");
        if (calls != 2) $fatal(1, "false prefix evaluated source");
        packet = 8'h36;
        result = sample() matches .branch &&& branch.lo == 6 ? branch.hi : 0;
        if (result != 3 || calls != 3) $fatal(1, "conditional binding");

        record_value = '{key: 7, lanes: '{8'h21, 8'h43}};
        if (bind_local(record_value) != 8'h43) $fatal(1, "automatic array binding");
        if (sample_record() matches .whole &&& whole.key == 7) begin
            record_value.lanes[1] = 0;
            result = whole.lanes[1];
        end else $fatal(1, "whole unpacked record binding");
        if (result != 8'h43 || calls != 4) $fatal(1, "unpacked source snapshot");
        if (record_value matches .*) result = 2;
        else $fatal(1, "unpacked wildcard");
        case (record_value) matches
            .first &&& first.key == 0: result = 0;
            .second &&& second.key == 7: result = second.lanes[0];
            default: result = 0;
        endcase
        if (result != 8'h21) $fatal(1, "whole record case/filter binding");

        choice = tagged valid 8'h96;
        if (choice matches .whole &&& whole matches tagged valid .payload)
            result = payload;
        else $fatal(1, "whole tagged binding followed by payload match");
        if (result != 8'h96) $fatal(1, "tagged binding payload");
        choice = 'x;
        if (choice matches .*) result = 3;
        else $fatal(1, "tagged wildcard must not check active tag");
        $display("PASS n08_whole_patterns");
        $finish(0);
    end
endmodule
