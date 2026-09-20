// llg-test-fixture: tests/fixtures/sim/loops/foreach_mixed_types.sv
// IEEE 1800-2009 12.7.3: integral element vectors and explicit singleton ranges.
module tb;
    typedef struct packed { logic [2:0] data; logic flag; } record_t;
    typedef enum logic [9:4] { ZERO = 0, ONE = 1 } enum_t;
    typedef byte byte_t;
    byte_t [1:0] bytes [1:0];
    int integers [0:1];
    record_t records [0:1];
    enum_t enums [0:1];
    logic scalar_bits [0:1];
    logic [7:7] singleton_bits [0:1];
    logic [1:0][3:2] only_packed;
    integer byte_visits;
    integer integer_visits;
    integer record_visits;
    integer enum_visits;
    integer enum_indices;
    integer scalar_visits;
    integer singleton_visits;
    integer packed_visits;

    initial begin
        byte_visits = 0;
        foreach (bytes[i,j,k]) begin
            bytes[i][j][k] = k % 2;
            byte_visits++;
        end
        integer_visits = 0;
        foreach (integers[i,j]) integer_visits++;
        record_visits = 0;
        foreach (records[i,j]) record_visits++;
        enum_visits = 0;
        enum_indices = 0;
        foreach (enums[i,j]) begin
            enum_visits++;
            enum_indices += j;
        end
        scalar_visits = 0;
        foreach (scalar_bits[i]) scalar_visits++;
        singleton_visits = 0;
        foreach (singleton_bits[i,j]) singleton_visits++;
        packed_visits = 0;
        foreach (only_packed[i,j]) packed_visits++;
        $display("bytes=%0d integers=%0d records=%0d enums=%0d enum_indices=%0d scalars=%0d singletons=%0d packed=%0d data=%h,%h",
                 byte_visits, integer_visits, record_visits, enum_visits, enum_indices,
                 scalar_visits, singleton_visits, packed_visits, bytes[1], bytes[0]);
    end
endmodule
