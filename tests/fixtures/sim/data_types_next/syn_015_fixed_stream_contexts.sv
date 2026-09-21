// llg-test-fixture: tests/fixtures/sim/data_types_next/syn_015_fixed_stream_contexts.sv
// LRM: IEEE 1800-2009 §§6.24.1, 6.24.3, 7.2, 11.4.14, and 13.5.2: fixed
// bit-stream casts and streams preserve leaf order through ref projections.
module tb;
    typedef logic [3:0] nibble_t;
    typedef nibble_t row_t [0:1];
    typedef row_t matrix_t [1:0];
    typedef struct {
        nibble_t tag;
        row_t lanes;
    } payload_t;
    typedef logic [7:0] word_t;
    typedef logic [11:0] payload_bits_t;

    row_t row;
    matrix_t matrix;
    payload_t payload;
    payload_t unpacked_payload;
    word_t word;
    payload_bits_t payload_bits;
    integer selector_calls;

    function automatic word_t flatten_row(const ref row_t source);
        flatten_row = word_t'(source);
    endfunction

    function automatic payload_bits_t flatten_payload(const ref payload_t source);
        flatten_payload = payload_bits_t'(source);
    endfunction

    function automatic word_t flatten_payload_lanes(const ref payload_t source);
        flatten_payload_lanes = word_t'(source.lanes);
    endfunction

    function automatic row_t unflatten_row(input word_t source);
        unflatten_row = row_t'(source);
    endfunction

    task automatic stream_row(ref row_t destination, input word_t source);
        {>>4{destination}} = source;
    endtask

    function automatic integer select_once;
        selector_calls = selector_calls + 1;
        select_once = 1;
    endfunction

    initial begin
        row[0] = 4'ha;
        row[1] = 4'hb;
        matrix[1] = row;
        matrix[0] = '{4'hc, 4'hd};
        payload.tag = 4'he;
        payload.lanes = row;

        word = flatten_row(row);
        if (word !== 8'hab) begin
            $display("FAIL syn_015 fixed ref cast");
            $finish;
        end
        row = unflatten_row(8'hcd);
        if (row[0] !== 4'hc || row[1] !== 4'hd) begin
            $display("FAIL syn_015 fixed return cast");
            $finish;
        end

        word = word_t'(matrix[1]);
        if (word !== 8'hab) begin
            $display("FAIL syn_015 selected row cast");
            $finish;
        end
        payload_bits = flatten_payload(payload);
        if (payload_bits !== 12'heab) begin
            $display("FAIL syn_015 nested aggregate cast");
            $finish;
        end
        if (flatten_payload_lanes(payload) !== 8'hab) begin
            $display("FAIL syn_015 selected aggregate member cast");
            $finish;
        end

        payload_bits = 12'h123;
        unpacked_payload = payload_t'(payload_bits);
        if (unpacked_payload.tag !== 4'h1 ||
            unpacked_payload.lanes[0] !== 4'h2 ||
            unpacked_payload.lanes[1] !== 4'h3) begin
            $display("FAIL syn_015 nested aggregate unpack");
            $finish;
        end

        stream_row(row, 8'h45);
        if (row[0] !== 4'h4 || row[1] !== 4'h5) begin
            $display("FAIL syn_015 ref stream destination");
            $finish;
        end

        row[0] = 4'h6;
        row[1] = 4'h7;
        selector_calls = 0;
        {>>{row[select_once()]}} = 4'h8;
        if (selector_calls !== 1 || row[0] !== 4'h6 || row[1] !== 4'h8) begin
            $display("FAIL syn_015 selected stream destination");
            $finish;
        end

        row[0] = 4'ha;
        row[1] = 4'hb;
        {<<4{row}} = {>>4{row}};
        if (row[0] !== 4'hb || row[1] !== 4'ha) begin
            $display("FAIL syn_015 overlapping stream snapshot");
            $finish;
        end

        $display("PASS syn_015_fixed_stream_contexts");
        $finish;
    end
endmodule
