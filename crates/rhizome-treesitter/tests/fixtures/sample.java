import java.util.List;
import java.util.Map;

public class UserService {
    private String name;
    private int age;

    public UserService(String name, int age) {
        this.name = name;
        this.age = age;
    }

    public String getName() {
        return name;
    }

    public void setAge(int age) {
        this.age = age;
    }
}

interface Repository {
    List<Object> findAll();
    Object findById(int id);
}

enum Status {
    ACTIVE,
    INACTIVE,
    PENDING
}
