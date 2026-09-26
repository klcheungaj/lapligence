// llg-test-fixture: tests/fixtures/sim/syn023_structure_patterns/recursive_runtime.sv
// IEEE 1800-2009 12.6, 12.6.2-12.6.3: recursive members and lexical bindings.
module tb;
    typedef struct packed {
        logic [3:0] code;
        bit signed [3:0] count;
    } word_t;
    typedef struct packed {
        word_t word;
        logic [7:0] tail;
    } packet_t;
    typedef struct {
        word_t rows[1:0];
        logic [6:0] mark;
    } table_t;
    typedef struct packed {
        logic [128:0] wide;
        bit [6:0] narrow;
    } wide_t;

    packet_t packet;
    table_t table_value;
    wide_t wide_value;
    int calls, checks, result;

    function automatic packet_t sample_packet();
        calls++;
        return packet;
    endfunction

    function automatic table_t sample_table();
        calls++;
        return table_value;
    endfunction

    initial begin
        packet = '{word: '{code: 4'ha, count: -4'sd2}, tail: 8'hc3};
        table_value = '{rows: '{'{code: 4'h1, count: 4'sd2},
                                 '{code: 4'h3, count: 4'sd4}}, mark: 7'h35};
        calls = 0;
        checks = 0;

        if (sample_packet() matches .whole &&& whole.word.code == 4'ha) begin
            packet = '0;
            if (whole.tail !== 8'hc3 || whole.word.count !== -4'sd2)
                $fatal(1, "whole packed snapshot");
            checks++;
        end else $fatal(1, "whole packed binding");
        if (calls != 1) $fatal(1, "whole packed source count");

        packet = '{word: '{code: 4'ha, count: -4'sd2}, tail: 8'hc3};
        if (packet matches .*) checks++;
        else $fatal(1, "packed wildcard");
        if (packet matches '{tail: 8'hc3, word: '{count: .n, code: 4'ha}}
            &&& n == -4'sd2) checks++;
        else $fatal(1, "named nested pattern");
        if (packet matches '{'{.code, .*}, .tail_value} &&&
            code == 4'ha && tail_value == 8'hc3) checks++;
        else $fatal(1, "positional nested pattern");

        if (sample_table() matches .record &&& record.rows[1].code == 4'h1) begin
            table_value.rows[1].code = 0;
            if (record.rows[1].code !== 4'h1 || record.mark !== 7'h35)
                $fatal(1, "whole unpacked snapshot");
            checks++;
        end else $fatal(1, "whole unpacked binding");
        if (calls != 2) $fatal(1, "whole unpacked source count");
        if (table_value matches .*) checks++;
        else $fatal(1, "unpacked wildcard");

        table_value.rows[1].code = 4'h1;
        if (table_value matches '{mark: 7'h35, rows: .bound_rows} &&&
            bound_rows[0].code == 4'h3 && bound_rows[1].count == 4'sd2)
            checks++;
        else $fatal(1, "array of structs member binding");
        if (table_value matches '{rows: .*, mark: .bound_mark} &&&
            bound_mark == 7'h35) checks++;
        else $fatal(1, "unpacked omitted and wildcard members");

        if (1'b0 &&& sample_packet() matches .unreached) $fatal(1, "false prefix");
        if (calls != 2) $fatal(1, "false prefix evaluated source");
        result = sample_packet() matches '{word: '{code: 4'ha}} &&&
                 1'b0 ? 1 : 0;
        if (result) $fatal(1, "false filter");
        if (calls != 3) $fatal(1, "filtered source count");
        checks++;

        result = packet matches .choice &&& choice.word.code == 4'ha
                 ? choice.tail : 0;
        if (result == 8'hc3) checks++;
        else $fatal(1, "conditional whole binding");

        wide_value = '{wide: {1'b1, 128'h1}, narrow: 7'h35};
        if (wide_value matches '{narrow: 7'h35, wide: .wide_member} &&&
            wide_member[128] && wide_member[0] &&
            wide_member[127:1] == 0) checks++;
        else $fatal(1, "wide member binding across limbs");

        if (checks != 11) $fatal(1, "checks=%0d", checks);
        $display("structure_patterns=pass checks=%0d calls=%0d", checks, calls);
        $finish(0);
    end
endmodule
