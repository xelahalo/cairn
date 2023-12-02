#include <stdio.h>

#define MULTIPLY(x, y) ((x) * (y))
#define SQUARE(x) MULTIPLY((x), (x))
#define CUBE(x) MULTIPLY((x), SQUARE(x))
#define QUARTER(x) MULTIPLY((x), CUBE(x))
#define EIGHTH(x) MULTIPLY((x), QUARTER(x))
#define SIXTEENTH(x) MULTIPLY((x), EIGHTH(x))
#define THIRTY_SECOND(x) MULTIPLY((x), SIXTEENTH(x))
#define SIXTY_FOURTH(x) MULTIPLY((x), THIRTY_SECOND(x))
#define ONE_TWENTY_EIGHTH(x) MULTIPLY((x), SIXTY_FOURTH(x))
#define TWO_FIFTY_SIXTH(x) MULTIPLY((x), ONE_TWENTY_EIGHTH(x))
#define FIVE_TWELFTH(x) MULTIPLY((x), TWO_FIFTY_SIXTH(x))
#define TEN_TWENTY_FOURTH(x) MULTIPLY((x), FIVE_TWELFTH(x))
#define TWENTY_FORTY_EIGHTH(x) MULTIPLY((x), TEN_TWENTY_FOURTH(x))
#define FORTY_NINETY_SIXTH(x) MULTIPLY((x), TWENTY_FORTY_EIGHTH(x))
#define EIGHTY_ONE_HUNDRED_NINETY_SECOND(x) MULTIPLY((x), FORTY_NINETY_SIXTH(x))
#define ONE_HUNDRED_SIXTY_THIRD(x) MULTIPLY((x), EIGHTY_ONE_HUNDRED_NINETY_SECOND(x))
#define THREE_HUNDRED_TWENTY_SEVENTH(x) MULTIPLY((x), ONE_HUNDRED_SIXTY_THIRD(x))

int main() {
    int result = 0;
    
    result = THREE_HUNDRED_TWENTY_SEVENTH(2);
    
    printf("Result: %d\n", result);
    
    return 0;
}

