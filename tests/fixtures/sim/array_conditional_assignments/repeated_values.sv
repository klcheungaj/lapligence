module tb;
    typedef logic [64:0] row_t [0:1];
    typedef logic [129:0] packed_row_t;
    row_t row, other;
    logic select, fill_bit;
    logic [7:0] counter;
    logic [87:0] gathered;
    logic [1039:0] combined;
    initial begin
        counter = 1;
        select = 1;
        fill_bit = 1;
        other = '{default: 65'b0};
        #1;
        gathered = {8'haa, counter++, counter++, counter++, counter++,
                    counter++, counter++, counter++, counter++, counter++, 8'hbb};
        if (gathered !== 88'haa010203040506070809bb || counter !== 10)
            $fatal(1, "repeated operands lost effects or order");
        row = select ? row_t'('{default: '{default: fill_bit}})
                     : row_t'('{default: '{default: 1'b0}});
        if (row[0] !== {65{1'b1}} || row[1] !== {65{1'b1}})
            $fatal(1, "known conditional lost repeated wide defaults");
        combined = {packed_row_t'(select ? row : other), packed_row_t'(select ? row : other),
                    packed_row_t'(select ? row : other), packed_row_t'(select ? row : other),
                    packed_row_t'(select ? row : other), packed_row_t'(select ? row : other),
                    packed_row_t'(select ? row : other), packed_row_t'(select ? row : other)};
        if (combined !== {1040{1'b1}})
            $fatal(1, "repeated array conditional lost known arms");
        #1;
        select = 1'bx;
        combined = {packed_row_t'(select ? row : other), packed_row_t'(select ? row : other),
                    packed_row_t'(select ? row : other), packed_row_t'(select ? row : other),
                    packed_row_t'(select ? row : other), packed_row_t'(select ? row : other),
                    packed_row_t'(select ? row : other), packed_row_t'(select ? row : other)};
        if (combined !== {1040{1'bx}})
            $fatal(1, "repeated array conditional lost ambiguous arms");
        row = select ? row_t'('{default: '{default: fill_bit}})
                     : row_t'('{default: '{default: 1'b0}});
        if (row[0] !== {65{1'bx}} || row[1] !== {65{1'bx}})
            $fatal(1, "ambiguous conditional lost wide element defaults");
        select = 0;
        row = select ? row_t'('{default: '{default: fill_bit}})
                     : row_t'('{default: '{default: 1'b0}});
        if (row[0] !== 65'b0 || row[1] !== 65'b0)
            $fatal(1, "false conditional lost repeated defaults");
        $display("repeated_values passed");
        $finish(0);
    end
endmodule
